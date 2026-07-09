//! Test oracle that runs a Noir program through Noir's ACVM/Brillig executor.
//!
//! Tests compare this decoded return value with the AST interpreter's result.
//!
//! Mirrors the compile-then-execute sequence in Noir's `tooling/artifact_cli/src/execution.rs`
//! (reimplemented here to avoid depending on the CLI crate).
//!
//! Under `goldilocks`, many programs cannot compile until the crypto stdlib supports the smaller
//! field, so asserting tests are currently gated to bn254.

use noirc_abi::input_parser::{Format, InputValue};
use noirc_abi::{InputMap, MAIN_RETURN_NAME};
use noirc_driver::{CompileOptions, compile_main};

use super::loader::NoirProject;
use super::validation_frontend::PackageSource;

/// Compile `project` with Noir's real pipeline and execute it, returning the decoded `main` return
/// value (`None` when `main` returns unit). Callers treat `Err` as "cannot judge this program".
pub(crate) fn noir_execute_return(
    project: &NoirProject,
    prover_toml: Option<&str>,
) -> Result<Option<InputValue>, String> {
    let (mut context, crate_id) = nargo::prepare_package(
        project.file_manager(),
        project.parsed_files(),
        project.get_only_crate(),
    );
    let (compiled, _warnings) =
        compile_main(&mut context, crate_id, &CompileOptions::default(), None)
            .map_err(|errs| format!("compile: {errs:?}"))?;

    // Prover.toml -> parameter inputs. The parser also yields a `return` entry (the corpus's
    // recorded output); drop it, since `encode` wants only `main`'s parameters.
    let mut input_map: InputMap = match prover_toml {
        Some(src) => Format::Toml
            .parse(src, &compiled.abi)
            .map_err(|e| format!("inputs: {e}"))?,
        None => InputMap::new(),
    };
    input_map.remove(MAIN_RETURN_NAME);

    let initial_witness = compiled
        .abi
        .encode(&input_map, None)
        .map_err(|e| format!("encode: {e}"))?;

    // Print foreign calls are discarded (output = Empty); real oracle calls are unhandled and error.
    let mut foreign_executor =
        nargo::foreign_calls::DefaultForeignCallBuilder::default().build::<acvm::FieldElement>();

    #[cfg(not(feature = "goldilocks"))]
    let solver = bn254_blackbox_solver::Bn254BlackBoxSolver;
    #[cfg(feature = "goldilocks")]
    let solver = acvm::blackbox_solver::StubbedBlackBoxSolver;

    let witness_stack = nargo::ops::execute_program(
        &compiled.program,
        initial_witness,
        &solver,
        &mut foreign_executor,
    )
    .map_err(|e| format!("execute: {e:?}"))?;

    let main_witness = &witness_stack
        .peek()
        .ok_or_else(|| "no witness produced".to_string())?
        .witness;
    let (_inputs, actual_return) = compiled
        .abi
        .decode(main_witness)
        .map_err(|e| format!("decode: {e}"))?;
    Ok(actual_return)
}
