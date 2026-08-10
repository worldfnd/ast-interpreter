//! Test support for producing a monomorphized Noir AST while tracking dependency diagnostics.

use std::collections::{BTreeMap, BTreeSet};

use fm::{FileId, FileManager};
use nargo::package::Package;
use noirc_abi::Abi;
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

/// A validation frontend failure with a human-readable diagnostic.
#[derive(Debug)]
pub(crate) enum ValidationError {
    Compile(String),
    DependencyCompileGap(String),
}

impl ValidationError {
    fn message(&self) -> &str {
        match self {
            ValidationError::Compile(m) | ValidationError::DependencyCompileGap(m) => m,
        }
    }

    pub(crate) fn is_dependency_compile_gap(&self) -> bool {
        matches!(self, ValidationError::DependencyCompileGap(_))
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
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
            ValidationError::Compile("expected a `main` function to validate".to_string())
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
        .map_err(|e| ValidationError::Compile(format!("{e:?}")))?;
    monomorphizer
        .process_queue()
        .map_err(|e| ValidationError::Compile(format!("{e:?}")))?;
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
                return Err(ValidationError::Compile(format!(
                    "Noir compiler error: {package_errors:?}"
                )));
            }
            Ok(tolerated.into_iter().map(|d| d.file).collect())
        }
        #[cfg(not(feature = "goldilocks"))]
        Err(diagnostics) => Err(ValidationError::Compile(format!(
            "Noir compiler error: {diagnostics:?}"
        ))),
    }
}

/// Reject monomorphized code from dependencies whose diagnostics were tolerated. Constants folded
/// during elaboration remain outside this provenance check.
fn reject_code_from_tolerated_files(
    file_manager: &FileManager,
    monomorphizer: &Monomorphizer,
    tolerated: &BTreeSet<FileId>,
) -> Result<(), ValidationError> {
    let poisoned: Vec<String> = monomorphizer
        .monomorphized_source_files()
        .intersection(tolerated)
        .map(|id| {
            file_manager
                .path(*id)
                .map_or_else(|| format!("{id:?}"), |p| p.display().to_string())
        })
        .collect();
    if poisoned.is_empty() {
        return Ok(());
    }
    Err(ValidationError::DependencyCompileGap(format!(
        "program reaches dependency code from files that failed elaboration under the chosen \
         field: {}",
        poisoned.join(", ")
    )))
}
