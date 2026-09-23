//! Bridge `Prover.toml` inputs into interpreter [`Value`]s.
//!
//! Noir's ABI parser reads every value in the program's field and yields an [`InputValue`] tree
//! keyed by parameter name; its scalar codec gives each leaf's element in that field, and we map
//! each value onto the monomorphized parameter [`Type`], using the matching [`AbiType`] for the
//! struct field ordering the lowered `Type::Tuple` loses. An integer input arrives as its
//! fixed-width two's complement pattern; the parser has already refused any value whose pattern
//! does not fit the field, so decoding that pattern is exact.

use std::collections::HashSet;
use std::rc::Rc;

use acvm::{FieldConfig, FieldId, FieldValue};
use num_bigint::BigInt;

use noirc_abi::{
    Abi, AbiType, MAIN_RETURN_NAME, Sign, encode_scalar,
    input_parser::{Format, InputValue},
};
use noirc_frontend::monomorphization::ast::{Program, Type};

use super::error::InterpretError;
use super::value::{IntValue, Value};

/// Parse `toml_src` against `abi` in `field`, which must be the field the program was compiled
/// under, and bind each value to `main`'s parameters in order.
pub fn inputs_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
    field: FieldId,
) -> Result<Vec<Value>, InterpretError> {
    // An unrepresentable recorded return must not prevent parsing the inputs.
    let parameters_only = Abi {
        return_type: None,
        ..abi.clone()
    };
    let map = parse_prover_toml(toml_src, &parameters_only, field)?;

    let main = super::main_function_of(program)?;

    let mut inputs = Vec::with_capacity(main.parameters.len());
    for (_, _, name, typ, _) in &main.parameters {
        let abi_type = abi
            .parameters
            .iter()
            .find(|p| &p.name == name)
            .map(|p| &p.typ)
            .ok_or_else(|| InterpretError::Internal(format!("no ABI parameter named '{name}'")))?;
        let input = map
            .get(name)
            .ok_or_else(|| InterpretError::Internal(format!("no parsed input for '{name}'")))?;
        inputs.push(value_from_input(input, abi_type, typ, field)?);
    }
    Ok(inputs)
}

/// Decode the expected `main` return value recorded in `Prover.toml` (the `return = ...` field), if
/// present, in `field` as [`inputs_from_prover_toml`] does. Noir's test corpus records this as the
/// program's known-correct output, so it is a ground-truth reference the interpreter's result can
/// be checked against.
pub fn expected_return_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
    field: FieldId,
) -> Result<Option<Value>, InterpretError> {
    let map = parse_prover_toml(toml_src, abi, field)?;
    let Some(input) = map.get(MAIN_RETURN_NAME) else {
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
    Ok(Some(value_from_input(
        input,
        abi_type,
        &main.return_type,
        field,
    )?))
}

fn parse_prover_toml(
    toml_src: &str,
    abi: &Abi,
    field: FieldId,
) -> Result<noirc_abi::InputMap, InterpretError> {
    Format::Toml
        .parse(toml_src, abi, FieldConfig::new(field))
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

/// Map one ABI [`InputValue`] read in `field` onto a monomorphized [`Type`], producing a [`Value`].
/// `abi_type` supplies the struct field ordering that the lowered `Type::Tuple` drops. Reused by
/// the executor oracle to decode Noir's ACVM return into a comparable `Value`.
pub(crate) fn value_from_input(
    input: &InputValue,
    abi_type: &AbiType,
    typ: &Type,
    field: FieldId,
) -> Result<Value, InterpretError> {
    // The element of a scalar, checked against its type and the field: this also guards values
    // the parser never saw, such as the ACVM returns the executor oracle decodes here.
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
