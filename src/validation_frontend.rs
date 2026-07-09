//! Test-support: drive Noir's frontend to a monomorphized AST for the interpreter to validate.
//!
//! Under `goldilocks`, dependency-only elaboration errors are tolerated until the crypto stdlib
//! supports the smaller field.

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

/// A failure of the validation frontend. The message is human-readable; tests match on its
/// content (e.g. the tolerated-file rejection contains "failed elaboration").
#[derive(Debug)]
pub(crate) struct ValidationError(pub String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
        .ok_or_else(|| ValidationError("expected a `main` function to validate".to_string()))?;
    let debug_type_tracker =
        DebugTypeTracker::build_from_debug_instrumenter(&DebugInstrumenter::default());
    // beta.22: `Monomorphizer::new` gained a `&FileMap` (arg 2) and an `Option<CrateId>`
    // `debug_crate_id` (arg 4); pass the crate's file map and `None`, matching Noir's own
    // non-debug `monomorphize` entry point.
    let mut monomorphizer = Monomorphizer::new(
        &mut context.def_interner,
        context.file_manager.as_file_map(),
        debug_type_tracker,
        None,
        false,
    );
    monomorphizer
        .compile_main(main)
        .map_err(|e| ValidationError(format!("{e:?}")))?;
    monomorphizer
        .process_queue()
        .map_err(|e| ValidationError(format!("{e:?}")))?;
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

/// Handle the whole-crate `check_crate` result, returning the set of files whose elaboration
/// errors were *tolerated*.
///
/// Under `goldilocks` the auto-injected bn254 crypto stdlib does not type-check (254-bit `Field`
/// constants, values >= p), so errors confined to files outside the user's package are tolerated
/// and their files returned; errors in the user's own package stay fatal. Note "outside the
/// package" covers *any* non-package crate, not just the stdlib — a broken user dependency is
/// tolerated too, bounded by the same invariant below. Tolerating an error is only sound together
/// with [`reject_code_from_tolerated_files`]: most elaboration type errors do not produce HIR
/// `Error` nodes — the wrongly-typed expression stays in the HIR — so reached broken code could
/// otherwise monomorphize *silently* into a wrong AST.
///
/// Under bn254 the stdlib is clean and any error is fatal (the strict behaviour); the returned
/// set is empty.
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
            // `is_bug()` diagnostics (frontend ICEs) are treated like errors: fatal in the
            // package, tolerated-but-tracked in dependencies — silently proceeding past an ICE
            // is the one thing worse than failing on it.
            let (package_errors, tolerated): (Vec<_>, Vec<_>) = diagnostics
                .into_iter()
                .filter(|d| d.is_error() || d.is_bug())
                .partition(|d| package_files.contains(&d.file));
            if !package_errors.is_empty() {
                return Err(ValidationError(format!(
                    "Noir compiler error: {package_errors:?}"
                )));
            }
            Ok(tolerated.into_iter().map(|d| d.file).collect())
        }
        #[cfg(not(feature = "goldilocks"))]
        Err(diagnostics) => Err(ValidationError(format!(
            "Noir compiler error: {diagnostics:?}"
        ))),
    }
}

/// Reject a monomorphized program that contains code from any file whose elaboration errors were
/// tolerated by [`tolerated_dependency_error_files`]. Such code may be silently mistyped, so a
/// program reaching it cannot be trusted — failing loudly here is what makes the tolerance sound.
///
/// Precise scope: the monomorphizer records the defining file of every function, global, trait
/// constant, and trait-impl associated constant it monomorphizes, so the resulting mono-AST
/// provably contains no *code or inlined constant* from a file that failed elaboration. Values
/// folded during elaboration itself (a dependency global used as an array length, comptime
/// evaluation) leave no monomorphization-time trace and are a documented residual risk until the
/// stdlib port removes the tolerated errors entirely.
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
    Err(ValidationError(format!(
        "program reaches code from files that failed elaboration under the chosen field (porting \
         them is the stdlib-port work): {}",
        poisoned.join(", ")
    )))
}
