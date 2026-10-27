//! Bridge entry-point inputs into interpreter [`Value`]s.
//!
//! Inputs go through [`noirc_abi::bytes`], the byte codec of entry-point values, which is the same
//! under every field: a `Prover.toml` is read by its parser and encoded into the parameter blob
//! a backend reads, and the blob is decoded and bound to the monomorphized parameter
//! [`Type`]s. The codec has already checked every value against its [`AbiType`], which also gives
//! the struct field order the lowered `Type::Tuple` loses.

use std::collections::HashSet;
use std::rc::Rc;

use acvm::FieldId;
use noirc_abi::bytes::{self, Inputs};
use noirc_abi::input_parser::Format;
use noirc_abi::{Abi, AbiType, Sign};
use noirc_frontend::monomorphization::ast::{Program, Type};

use super::error::InterpretError;
use super::value::{IntValue, Value};

/// Parse `toml_src` against `abi` and bind each value to `main`'s parameters in order, through
/// the parameter blob [`inputs_from_bytes`] reads.
pub fn inputs_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
) -> Result<Vec<Value>, InterpretError> {
    let inputs = parse_prover_toml(toml_src, abi)?;
    let blob = bytes::encode_params(abi, &inputs.params)
        .map_err(|e| InterpretError::Internal(format!("parsed inputs do not encode: {e}")))?;
    inputs_from_bytes(program, abi, &blob)
}

/// Decode `blob`, the parameter blob of `abi`, and bind each value to `main`'s parameters in
/// order.
pub fn inputs_from_bytes(
    program: &Program,
    abi: &Abi,
    blob: &[u8],
) -> Result<Vec<Value>, InterpretError> {
    let main = super::main_function_of(program)?;
    let parameters: Vec<(&str, &Type)> = main
        .parameters
        .iter()
        .map(|(_, _, name, typ, _)| (name.as_str(), typ.as_ref()))
        .collect();
    bind_params(abi, &parameters, blob)
}

/// Decode `blob` and bind each value to the parameter of the same position in `parameters`, the
/// name and monomorphized type of each parameter of the entry point `abi` describes.
pub(crate) fn bind_params(
    abi: &Abi,
    parameters: &[(&str, &Type)],
    blob: &[u8],
) -> Result<Vec<Value>, InterpretError> {
    let values = bytes::decode_params(abi, blob)
        .map_err(|e| InterpretError::InvalidInput(format!("the parameter blob is refused: {e}")))?;
    if parameters.len() != abi.parameters.len() {
        return Err(InterpretError::Internal(format!(
            "the ABI has {} parameters, the entry point {}",
            abi.parameters.len(),
            parameters.len()
        )));
    }
    parameters
        .iter()
        .zip(&abi.parameters)
        .zip(&values)
        .map(|(((name, typ), param), value)| {
            if param.name != *name {
                return Err(InterpretError::Internal(format!(
                    "the ABI names parameter '{}' where the entry point has '{name}'",
                    param.name
                )));
            }
            bind(value, &param.typ, typ)
        })
        .collect()
}

/// Decode the expected `main` return value recorded in `Prover.toml` (the `return` key), if
/// present. Noir's test corpus records this as the program's known-correct output, so it is a
/// ground-truth reference the interpreter's result can be checked against.
pub fn expected_return_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
) -> Result<Option<Value>, InterpretError> {
    let Some(ret) = parse_prover_toml(toml_src, abi)?.ret else {
        return Ok(None);
    };
    let main = super::main_function_of(program)?;
    let abi_type = abi
        .return_type
        .as_ref()
        .map(|r| &r.abi_type)
        .ok_or_else(|| {
            InterpretError::Internal(
                "Prover.toml parsed a return value but the ABI declares no return type".to_string(),
            )
        })?;
    bind(&ret, abi_type, &main.return_type).map(Some)
}

pub(crate) fn parse_prover_toml(toml_src: &str, abi: &Abi) -> Result<Inputs, InterpretError> {
    bytes::parse(&Format::Toml, toml_src, abi)
        .map_err(|e| InterpretError::InvalidInput(format!("failed to parse Prover.toml: {e}")))
}

/// Check the invariants the interpreter relies on but cannot restore from a caller-built [`Value`]:
/// every `Field` belongs to `field`, and every integer is canonical for its own type. [`IntValue`]'s
/// members are public, so an input can be spelled outside the range its width and signedness name.
pub(crate) fn validate_inputs(inputs: &[Value], field: FieldId) -> Result<(), InterpretError> {
    fn check(
        value: &Value,
        field: FieldId,
        seen: &mut HashSet<*const std::cell::RefCell<Value>>,
    ) -> Result<(), InterpretError> {
        let cells = match value {
            Value::Field(value) if value.field() != field => {
                return Err(InterpretError::InvalidInput(format!(
                    "Field input belongs to {}, but the program uses {field}",
                    value.field()
                )));
            }
            Value::Int(int) => return check_int(int),
            Value::Array(values)
            | Value::FmtStr {
                captures: values, ..
            } => {
                return values
                    .iter()
                    .try_for_each(|value| check(value, field, seen));
            }
            Value::Tuple(cells) => cells.as_slice(),
            Value::Ref(cell, _) => std::slice::from_ref(cell),
            _ => return Ok(()),
        };
        for cell in cells {
            // Shared or cyclic references need only one visit per cell.
            if seen.insert(Rc::as_ptr(cell)) {
                let value = cell.try_borrow().map_err(|_| {
                    InterpretError::InvalidInput("input cell is mutably borrowed".to_string())
                })?;
                check(&value, field, seen)?;
            }
        }
        Ok(())
    }

    let mut seen = HashSet::new();
    inputs
        .iter()
        .try_for_each(|input| check(input, field, &mut seen))
}

fn check_int(int: &IntValue) -> Result<(), InterpretError> {
    let sign = if int.signed { 'i' } else { 'u' };
    // A zero-width type holds no values at all, and `range` would underflow computing `bits - 1`.
    if int.bits == 0 {
        return Err(InterpretError::InvalidInput(format!(
            "integer input declares the empty type {sign}0"
        )));
    }
    let (min, max) = IntValue::range(int.signed, int.bits);
    if int.value < min || int.value > max {
        return Err(InterpretError::InvalidInput(format!(
            "integer input {} is not a value of {sign}{}",
            int.value, int.bits
        )));
    }
    Ok(())
}

/// Bind one decoded value of `abi_type` to the monomorphized `typ`. Both types come from one
/// compilation and the codec has checked the value against `abi_type`, so any disagreement is an
/// internal error.
pub(crate) fn bind(
    value: &bytes::Value,
    abi_type: &AbiType,
    typ: &Type,
) -> Result<Value, InterpretError> {
    let disagreement = || {
        InterpretError::Internal(format!(
            "ABI type {abi_type:?} does not describe {typ:?}, or the decoded value is not one of it"
        ))
    };
    let bind_each = |elements: &[bytes::Value], types: Vec<(&AbiType, &Type)>| {
        if elements.len() != types.len() {
            return Err(disagreement());
        }
        elements
            .iter()
            .zip(types)
            .map(|(element, (abi_type, typ))| bind(element, abi_type, typ))
            .collect::<Result<Vec<_>, _>>()
    };
    match (value, abi_type, typ) {
        (
            bytes::Value::Int(int),
            AbiType::Integer { sign, width },
            Type::Integer(signedness, bits),
        ) if (*sign == Sign::Signed) == signedness.is_signed() && width == bits => {
            Ok(Value::Int(IntValue {
                signed: signedness.is_signed(),
                bits: *bits,
                value: int.clone(),
            }))
        }
        (bytes::Value::Bool(b), AbiType::Boolean, Type::Bool) => Ok(Value::Bool(*b)),
        (bytes::Value::Str(s), AbiType::String { length }, Type::String(n)) if length == n => {
            Ok(Value::Str(s.clone()))
        }
        (
            bytes::Value::Seq(elements),
            AbiType::Array {
                length,
                typ: element_abi,
            },
            Type::Array(n, element_type),
        ) if length == n => {
            let types = vec![(element_abi.as_ref(), element_type.as_ref()); *n as usize];
            bind_each(elements, types).map(Value::Array)
        }
        (bytes::Value::Seq(elements), AbiType::Tuple { fields }, Type::Tuple(types))
            if fields.len() == types.len() =>
        {
            bind_each(elements, fields.iter().zip(types).collect()).map(Value::tuple)
        }
        (bytes::Value::Seq(elements), AbiType::Struct { fields, .. }, Type::Tuple(types))
            if fields.len() == types.len() =>
        {
            let fields = fields.iter().map(|(_, abi_type)| abi_type);
            bind_each(elements, fields.zip(types).collect()).map(Value::tuple)
        }
        _ => Err(disagreement()),
    }
}
