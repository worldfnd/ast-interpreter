//! Test support for producing a monomorphized Noir AST while tracking dependency diagnostics.

use std::collections::{BTreeMap, BTreeSet};

use fm::{FileId, FileManager};
use nargo::package::Package;
use noirc_abi::Abi;
use noirc_errors::CustomDiagnostic;
use noirc_frontend::debug::DebugInstrumenter;
use noirc_frontend::graph::CrateId;
use noirc_frontend::hir::{Context, ParsedFiles};
use noirc_frontend::monomorphization::Monomorphizer;
use noirc_frontend::monomorphization::ast::Program;
use noirc_frontend::monomorphization::debug_types::DebugTypeTracker;

/// The package data Noir's [`nargo::prepare_package`] needs.
pub(crate) trait PackageSource {
    fn file_manager(&self) -> &FileManager;
    fn parsed_files(&self) -> &ParsedFiles;
    fn get_only_crate(&self) -> &Package;
}

/// A monomorphized program plus its ABI, ready for the interpreter and the input bridge.
pub(crate) struct Validated {
    pub program: Program,
    pub abi: Abi,
}

/// A validation frontend failure. `summary` is the diagnostic text alone (messages without spans
/// or file ids), stable across compiler revisions that only move code around; `detail` is the full
/// debug rendering for triage.
#[derive(Debug)]
pub(crate) enum ValidationError {
    Compile { summary: String, detail: String },
    DependencyCompileGap { summary: String, detail: String },
}

impl ValidationError {
    fn compile(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        ValidationError::Compile {
            summary: summary.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn summary(&self) -> &str {
        match self {
            ValidationError::Compile { summary, .. }
            | ValidationError::DependencyCompileGap { summary, .. } => summary,
        }
    }

    pub(crate) fn detail(&self) -> &str {
        match self {
            ValidationError::Compile { detail, .. }
            | ValidationError::DependencyCompileGap { detail, .. } => detail,
        }
    }

    pub(crate) fn is_dependency_compile_gap(&self) -> bool {
        matches!(self, ValidationError::DependencyCompileGap { .. })
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.summary())
    }
}

/// The messages of the error and bug diagnostics in `diagnostics`, joined into one line.
fn diagnostic_summary(diagnostics: &[CustomDiagnostic]) -> String {
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.is_error() || d.is_bug())
        .map(|d| d.message.as_str())
        .collect();
    messages.join(" | ")
}

impl std::error::Error for ValidationError {}

/// Run the Noir frontend through monomorphization and return the mono-AST + ABI.
///
/// Under `goldilocks`, dependency-only elaboration errors are tolerated if no rejected code reaches
/// the monomorphized program.
pub(crate) fn compile_for_validation(
    source: &impl PackageSource,
) -> Result<Validated, ValidationError> {
    let (mut context, crate_id) = nargo::prepare_package(
        source.file_manager(),
        source.parsed_files(),
        source.get_only_crate(),
    );

    let check_result = noirc_driver::check_crate(
        &mut context,
        crate_id,
        &noirc_driver::CompileOptions::default(),
    );
    let tolerated_files = tolerated_dependency_error_files(&context, crate_id, check_result)?;

    let main = context
        .get_main_function(context.root_crate_id())
        .ok_or_else(|| {
            let message = "expected a `main` function to validate";
            ValidationError::compile(message, message)
        })?;
    let debug_type_tracker =
        DebugTypeTracker::build_from_debug_instrumenter(&DebugInstrumenter::default());
    // Match Noir's non-debug monomorphization entry point.
    let mut monomorphizer = Monomorphizer::new(
        &mut context.def_interner,
        context.file_manager.as_file_map(),
        debug_type_tracker,
        None,
        false,
    );
    monomorphizer
        .compile_main(main)
        .map_err(monomorphization_error)?;
    monomorphizer
        .process_queue()
        .map_err(monomorphization_error)?;
    reject_code_from_tolerated_files(&context.file_manager, &monomorphizer, &tolerated_files)?;
    let program = monomorphizer.into_program();

    let abi = noirc_driver::gen_abi(
        &context,
        &main,
        program.return_visibility(),
        BTreeMap::default(),
    );

    Ok(Validated { program, abi })
}

/// Return dependency files with tolerated diagnostics. Package diagnostics remain fatal, and
/// callers must reject monomorphized code originating from a tolerated file.
#[cfg_attr(not(feature = "goldilocks"), allow(unused_variables))]
fn tolerated_dependency_error_files(
    context: &Context,
    crate_id: CrateId,
    check_result: noirc_driver::CompilationResult<()>,
) -> Result<BTreeSet<FileId>, ValidationError> {
    match check_result {
        Ok(_) => Ok(BTreeSet::new()),
        #[cfg(feature = "goldilocks")]
        Err(diagnostics) => {
            let package_files = context.crate_files(&crate_id);
            // Track dependency ICEs too; proceeding past an untracked ICE is unsound.
            let (package_errors, tolerated): (Vec<_>, Vec<_>) = diagnostics
                .into_iter()
                .filter(|d| d.is_error() || d.is_bug())
                .partition(|d| package_files.contains(&d.file));
            if !package_errors.is_empty() {
                return Err(ValidationError::compile(
                    diagnostic_summary(&package_errors),
                    format!("Noir compiler error: {package_errors:?}"),
                ));
            }
            Ok(tolerated.into_iter().map(|d| d.file).collect())
        }
        #[cfg(not(feature = "goldilocks"))]
        Err(diagnostics) => Err(ValidationError::compile(
            diagnostic_summary(&diagnostics),
            format!("Noir compiler error: {diagnostics:?}"),
        )),
    }
}

fn monomorphization_error(
    error: noirc_frontend::monomorphization::errors::MonomorphizationError,
) -> ValidationError {
    let detail = format!("{error:?}");
    let summary = CustomDiagnostic::from(error).message;
    ValidationError::compile(summary, detail)
}

/// Reject monomorphized code from dependencies whose diagnostics were tolerated. Constants folded
/// during elaboration remain outside this provenance check.
fn reject_code_from_tolerated_files(
    file_manager: &FileManager,
    monomorphizer: &Monomorphizer,
    tolerated: &BTreeSet<FileId>,
) -> Result<(), ValidationError> {
    let mut poisoned: Vec<String> = monomorphizer
        .monomorphized_source_files()
        .intersection(tolerated)
        .map(|id| {
            file_manager
                .path(*id)
                .map_or_else(|| format!("{id:?}"), |p| p.display().to_string())
        })
        .collect();
    poisoned.sort();
    if poisoned.is_empty() {
        return Ok(());
    }
    let message = format!(
        "program reaches dependency code from files that failed elaboration under the chosen \
         field: {}",
        poisoned.join(", ")
    );
    Err(ValidationError::DependencyCompileGap {
        summary: message.clone(),
        detail: message,
    })
}
