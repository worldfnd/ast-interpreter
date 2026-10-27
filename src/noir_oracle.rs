//! Test oracle that runs a Noir program through Noir's ACVM/Brillig executor, so tests can compare
//! its decoded return value with the interpreter's. Mirrors the compile-then-execute sequence in
//! Noir's `tooling/artifact_cli/src/execution.rs`, reimplemented to avoid the CLI crate dependency.
//!
//! Without `bn254-crypto` the executor gets a stub black-box solver, so curve and Poseidon2
//! opcodes fail.

use acvm::{FieldConfig, FieldId, FieldValue};
use noirc_abi::input_parser::{Format, InputValue};
use noirc_abi::{AbiType, InputMap, MAIN_RETURN_NAME, Sign, encode_scalar};
use noirc_driver::{CompileOptions, compile_main};
use noirc_frontend::monomorphization::ast::Type;
use num_bigint::BigInt;

use super::error::InterpretError;
use super::loader::NoirProject;
use super::validation_frontend::PackageSource;
use super::value::{IntValue, Value};

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

    // Prover.toml -> parameter inputs, in the linked field the executor computes in. The parser
    // also yields a `return` entry (the corpus's recorded output); drop it, since `encode` wants
    // only `main`'s parameters.
    let mut input_map: InputMap = match prover_toml {
        Some(src) => Format::Toml
            .parse(src, &compiled.abi, FieldConfig::linked())
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

    #[cfg(feature = "bn254-crypto")]
    let solver = bn254_blackbox_solver::Bn254BlackBoxSolver;
    #[cfg(not(feature = "bn254-crypto"))]
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

/// Whether the ABI describes the scalar `typ` the program declares. Both come from one compilation,
/// so a mismatch is an internal error rather than a value checked against the wrong type.
fn scalar_abi_matches(abi_type: &AbiType, typ: &Type) -> bool {
    match (abi_type, typ) {
        (AbiType::Field, Type::Field) | (AbiType::Boolean, Type::Bool) => true,
        (AbiType::Integer { sign, width }, Type::Integer(signedness, bits)) => {
            (*sign == Sign::Signed) == signedness.is_signed() && width == bits
        }
        _ => false,
    }
}

/// Map one field-element ABI [`InputValue`] in `field` onto a monomorphized [`Type`], producing a
/// [`Value`]: the executor's return, which the ACVM decodes as field elements. `abi_type` supplies
/// the struct field ordering that the lowered `Type::Tuple` drops.
pub(crate) fn value_from_input(
    input: &InputValue,
    abi_type: &AbiType,
    typ: &Type,
    field: FieldId,
) -> Result<Value, InterpretError> {
    // The element of a scalar, checked against its type and the field.
    let element = || {
        encode_scalar(input, abi_type, FieldConfig::new(field)).map_err(|e| {
            InterpretError::InvalidInput(format!("input does not fit {abi_type:?}: {e}"))
        })
    };
    match (input, typ) {
        (InputValue::Field(_), Type::Field | Type::Integer(..) | Type::Bool)
            if !scalar_abi_matches(abi_type, typ) =>
        {
            Err(InterpretError::Internal(format!(
                "ABI type {abi_type:?} does not describe {typ:?}"
            )))
        }
        (InputValue::Field(_), Type::Field) => {
            let element = element()?;
            FieldValue::try_from_biguint(element, field)
                .map(Value::Field)
                .ok_or_else(|| {
                    InterpretError::Internal(
                        "an encoded element is not below its modulus".to_string(),
                    )
                })
        }
        (InputValue::Field(_), Type::Integer(signedness, bits)) => Ok(Value::Int(
            IntValue::canonical(signedness.is_signed(), *bits, BigInt::from(element()?)),
        )),
        (InputValue::Field(_), Type::Bool) => Ok(Value::Bool(element()? == 1u8.into())),
        (InputValue::Vec(elements), Type::Array(length, element_type)) => {
            let AbiType::Array {
                length: abi_length,
                typ: element_abi,
            } = abi_type
            else {
                return Err(InterpretError::Internal(format!(
                    "ABI type {abi_type:?} is not an array"
                )));
            };
            if abi_length != length {
                return Err(InterpretError::Internal(format!(
                    "array ABI length is {abi_length}, type expects {length}"
                )));
            }
            if elements.len() != *length as usize {
                return Err(InterpretError::InvalidInput(format!(
                    "array input has {} elements, type expects {length}",
                    elements.len()
                )));
            }
            let values = elements
                .iter()
                .map(|element| value_from_input(element, element_abi, element_type, field))
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(values))
        }
        (InputValue::Vec(elements), Type::Tuple(types)) => {
            let AbiType::Tuple { fields } = abi_type else {
                return Err(InterpretError::Internal(format!(
                    "ABI type {abi_type:?} is not a tuple"
                )));
            };

            if fields.len() != types.len() {
                return Err(InterpretError::Internal(format!(
                    "tuple ABI has {} fields, type expects {}",
                    fields.len(),
                    types.len()
                )));
            }
            if elements.len() != types.len() {
                return Err(InterpretError::InvalidInput(format!(
                    "tuple input has {} elements, type expects {}",
                    elements.len(),
                    types.len()
                )));
            }
            let values = fields
                .iter()
                .zip(types)
                .zip(elements)
                .map(|((field_abi, typ), element)| value_from_input(element, field_abi, typ, field))
                .collect::<Result<_, _>>()?;
            Ok(Value::tuple(values))
        }
        (InputValue::Struct(map), Type::Tuple(types)) => {
            // `fields` is declaration-ordered (fields[i] matches types[i]); the input map is a
            // BTreeMap (alphabetical), so look each field up by name rather than iterate it.
            let AbiType::Struct { fields, .. } = abi_type else {
                return Err(InterpretError::Internal(format!(
                    "ABI type {abi_type:?} is not a struct"
                )));
            };
            if fields.len() != types.len() {
                return Err(InterpretError::Internal(format!(
                    "struct ABI has {} fields, type expects {}",
                    fields.len(),
                    types.len()
                )));
            }
            let values = fields
                .iter()
                .zip(types)
                .map(|((name, field_abi), typ)| {
                    let value = map.get(name).ok_or_else(|| {
                        InterpretError::Internal(format!("ABI struct has no field '{name}'"))
                    })?;
                    value_from_input(value, field_abi, typ, field)
                })
                .collect::<Result<_, _>>()?;
            Ok(Value::tuple(values))
        }
        (InputValue::String(s), Type::String(_)) => Ok(Value::Str(s.clone().into_bytes())),
        (input, typ) => Err(InterpretError::Unsupported(format!(
            "input value {input:?} for parameter type {typ:?}"
        ))),
    }
}
