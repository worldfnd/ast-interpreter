//! Test support for producing a monomorphized Noir AST.

use std::collections::BTreeMap;

use acvm::FieldId;
use fm::FileManager;
use nargo::package::Package;
use noirc_abi::Abi;
use noirc_errors::CustomDiagnostic;
use noirc_frontend::debug::DebugInstrumenter;
use noirc_frontend::hir::{Context, FunctionNameMatch, ParsedFiles};
use noirc_frontend::monomorphization::Monomorphizer;
use noirc_frontend::monomorphization::ast::Program;
use noirc_frontend::monomorphization::debug_types::DebugTypeTracker;
use noirc_frontend::node_interner::FuncId;
use noirc_frontend::token::TestScope;
use sha2::{Digest, Sha256};

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
    /// The field the program was compiled under, taken from the monomorphizer's own label rather
    /// than from what was asked for.
    pub field_id: FieldId,
}

/// A frontend failure with a stable summary and diagnostic detail.
#[derive(Debug)]
pub(crate) struct ValidationError {
    summary: String,
    detail: String,
}

impl ValidationError {
    fn new(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        ValidationError {
            summary: summary.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.summary())
    }
}

impl std::error::Error for ValidationError {}

/// Bound displayed diagnostics when a dependency produces thousands of errors.
const RECORDED_DIAGNOSTICS: usize = 8;

/// Abbreviate long summaries while retaining a fingerprint of every error message.
fn diagnostic_summary(diagnostics: &[CustomDiagnostic]) -> String {
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.is_error() || d.is_bug())
        .map(|d| d.message.as_str())
        .collect();
    let mut summary = messages[..messages.len().min(RECORDED_DIAGNOSTICS)].join(" | ");
    if messages.len() > RECORDED_DIAGNOSTICS {
        let full_summary = super::diff::normalize_text(&messages.join(" | "));
        summary.push_str(&format!(
            " | ... and {} more (sha256={:x})",
            messages.len() - RECORDED_DIAGNOSTICS,
            Sha256::digest(full_summary.as_bytes())
        ));
    }
    summary
}

fn diagnostic_detail(diagnostics: &[CustomDiagnostic]) -> String {
    let shown = &diagnostics[..diagnostics.len().min(RECORDED_DIAGNOSTICS)];
    let mut detail = format!("Noir compiler error: {shown:?}");
    if diagnostics.len() > RECORDED_DIAGNOSTICS {
        detail.push_str(&format!(
            " ... and {} more",
            diagnostics.len() - RECORDED_DIAGNOSTICS
        ));
    }
    detail
}

/// Prepare `source` and run the frontend on it under `field`, rejecting every error whichever
/// file it lands in. `generic_builtins` is Noir's benchmark mode: the standard library's
/// field-generic builtins in place of their `field`-specific twins.
fn check_package<'s>(
    source: &'s impl PackageSource,
    field: FieldId,
    generic_builtins: bool,
) -> Result<Context<'s, 's>, ValidationError> {
    let (mut context, crate_id) = nargo::prepare_package(
        source.file_manager(),
        source.parsed_files(),
        source.get_only_crate(),
    );
    let options = noirc_driver::CompileOptions {
        field,
        generic_builtins,
        ..noirc_driver::CompileOptions::default()
    };
    noirc_driver::check_crate(&mut context, crate_id, &options).map_err(|diagnostics| {
        ValidationError::new(
            diagnostic_summary(&diagnostics),
            diagnostic_detail(&diagnostics),
        )
    })?;
    Ok(context)
}

/// Monomorphize `entry` out of a checked crate. The output carries the field it was compiled
/// under, which must be the one asked for.
fn monomorphize(
    context: &mut Context,
    entry: FuncId,
    field: FieldId,
) -> Result<(Program, FieldId), ValidationError> {
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
        .compile_main(entry)
        .map_err(monomorphization_error)?;
    monomorphizer
        .process_queue()
        .map_err(monomorphization_error)?;
    let output = monomorphizer.into_output();
    assert_eq!(
        output.field_id, field,
        "ICE: asked for a {field} program and got a {} one",
        output.field_id
    );
    Ok((output.program, output.field_id))
}

/// Produce the mono-AST and ABI of `main`.
pub(crate) fn compile_for_validation(
    source: &impl PackageSource,
    field: FieldId,
) -> Result<Validated, ValidationError> {
    compile_for_validation_with(source, field, false)
}

/// [`compile_for_validation`] with Noir's benchmark mode chosen: with `generic_builtins`, the
/// standard library's field-generic builtins replace their `field`-specific twins.
pub(crate) fn compile_for_validation_with(
    source: &impl PackageSource,
    field: FieldId,
    generic_builtins: bool,
) -> Result<Validated, ValidationError> {
    let mut context = check_package(source, field, generic_builtins)?;
    let main = context
        .get_main_function(context.root_crate_id())
        .ok_or_else(|| {
            let message = "expected a `main` function to validate";
            ValidationError::new(message, message)
        })?;
    let (program, field_id) = monomorphize(&mut context, main, field)?;
    let abi = noirc_driver::gen_abi(
        &context,
        &main,
        program.return_visibility(),
        BTreeMap::default(),
    );
    Ok(Validated {
        program,
        abi,
        field_id,
    })
}

/// One `#[test]` of the standard library, monomorphized under the field `source` was checked for.
pub(crate) struct StdlibTest {
    pub name: String,
    pub scope: TestScope,
    pub program: Result<Program, ValidationError>,
}

/// Every argument-less `#[test]` the standard library keeps under `field`.
pub(crate) fn stdlib_tests(
    source: &impl PackageSource,
    field: FieldId,
) -> Result<Vec<StdlibTest>, ValidationError> {
    let mut context = check_package(source, field, false)?;
    let stdlib = *context.stdlib_crate_id();
    let tests =
        context.get_all_test_functions_in_crate_matching(&stdlib, &FunctionNameMatch::Anything);
    Ok(tests
        .into_iter()
        .filter(|(_, test)| !test.has_arguments)
        .map(|(name, test)| StdlibTest {
            name,
            scope: test.scope,
            program: monomorphize(&mut context, test.id, field).map(|(program, _)| program),
        })
        .collect())
}

fn monomorphization_error(
    error: noirc_frontend::monomorphization::errors::MonomorphizationError,
) -> ValidationError {
    let detail = format!("{error:?}");
    let summary = CustomDiagnostic::from(error).message;
    ValidationError::new(summary, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_diagnostics_still_distinguish_later_errors() {
        let mut diagnostics = vec![
            CustomDiagnostic::from_message("type error", Default::default());
            RECORDED_DIAGNOSTICS + 1
        ];
        let before = diagnostic_summary(&diagnostics);
        diagnostics.last_mut().unwrap().message = "unresolved name".into();
        assert_ne!(before, diagnostic_summary(&diagnostics));
    }
}
