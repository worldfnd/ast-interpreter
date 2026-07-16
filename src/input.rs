//! Bridge `Prover.toml` inputs into interpreter [`Value`]s.
//!
//! Noir's ABI parser yields an [`InputValue`] tree keyed by parameter name; we map each value onto
//! the monomorphized parameter [`Type`], using the matching [`AbiType`] for the struct field
//! ordering the lowered `Type::Tuple` loses. Integer inputs are decoded from their two's-complement
//! field encoding and range-checked against the declared width, since the ABI parser only bounds
//! them by the field modulus.

use acvm::{AcirField, FieldElement};
use num_bigint::BigInt;

use noirc_abi::{
    Abi, AbiType, MAIN_RETURN_NAME,
    input_parser::{Format, InputValue},
};
use noirc_frontend::monomorphization::ast::{Program, Type};

use super::error::InterpretError;
use super::value::{IntValue, Value, field_to_bigint};

/// Parse `toml_src` against `abi` and bind each value to `main`'s parameters in order.
pub fn inputs_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
) -> Result<Vec<Value>, InterpretError> {
    let map = Format::Toml
        .parse(toml_src, abi)
        .map_err(|e| InterpretError::Internal(format!("failed to parse Prover.toml: {e}")))?;

    let main = program
        .functions
        .first()
        .ok_or_else(|| InterpretError::Internal("program has no functions".to_string()))?;

    let mut inputs = Vec::with_capacity(main.parameters.len());
    for (_, _, name, typ, _) in &main.parameters {
        let input = map
            .get(name)
            .ok_or_else(|| InterpretError::Internal(format!("missing input '{name}'")))?;
        // Match the ABI parameter by name, not position — the ABI order is not guaranteed to align
        // with the monomorphized parameter order.
        let abi_type = abi
            .parameters
            .iter()
            .find(|p| &p.name == name)
            .map(|p| &p.typ)
            .ok_or_else(|| InterpretError::Internal(format!("no ABI parameter named '{name}'")))?;
        inputs.push(value_from_input(input, abi_type, typ)?);
    }
    Ok(inputs)
}

/// Decode the expected `main` return value recorded in `Prover.toml` (the `return = ...` field), if
/// present. Noir's test corpus records this as the program's known-correct output, so it is a
/// ground-truth reference the interpreter's result can be checked against.
pub fn expected_return_from_prover_toml(
    program: &Program,
    abi: &Abi,
    toml_src: &str,
) -> Result<Option<Value>, InterpretError> {
    let map = Format::Toml
        .parse(toml_src, abi)
        .map_err(|e| InterpretError::Internal(format!("failed to parse Prover.toml: {e}")))?;
    let Some(input) = map.get(MAIN_RETURN_NAME) else {
        return Ok(None);
    };
    let main = program
        .functions
        .first()
        .ok_or_else(|| InterpretError::Internal("program has no functions".to_string()))?;
    let abi_type = abi
        .return_type
        .as_ref()
        .map(|r| &r.abi_type)
        .ok_or_else(|| InterpretError::Internal("ABI has no return type".to_string()))?;
    Ok(Some(value_from_input(input, abi_type, &main.return_type)?))
}

/// Map one ABI [`InputValue`] onto a monomorphized [`Type`], producing a [`Value`]. `abi_type`
/// supplies the struct field ordering that the lowered `Type::Tuple` drops. Reused by the executor
/// oracle to decode Noir's ACVM return into a comparable `Value`.
pub(crate) fn value_from_input(
    input: &InputValue,
    abi_type: &AbiType,
    typ: &Type,
) -> Result<Value, InterpretError> {
    match (input, typ) {
        (InputValue::Field(field), Type::Field) => Ok(Value::Field(*field)),
        (InputValue::Field(field), Type::Integer(signedness, bits)) => {
            let width = bits.bit_size();
            // noirc_abi encodes signed ints as two's complement (2^width + x) mod p; when
            // 2^width > p the map collides (Goldilocks i64 -1 and +4294967294 share a field
            // element), so the value is already lost and cannot be recovered here.
            if signedness.is_signed() && (BigInt::from(1u8) << width as usize) > field_modulus() {
                return Err(InterpretError::Unsupported(format!(
                    "signed {width}-bit ABI input is not representable in this field"
                )));
            }
            // The ABI parser only checks `value < field_modulus`, not `value < 2^bits`, so reject a
            // valid-but-oversized input rather than silently truncate. A valid witness holds the
            // value's two's-complement bit pattern in `[0, 2^bits)`.
            let raw = field_to_bigint(field);
            if raw.bits() > width as u64 {
                return Err(InterpretError::Type(format!(
                    "integer input does not fit a {width}-bit type"
                )));
            }
            Ok(Value::Int(IntValue::canonical(
                signedness.is_signed(),
                width,
                raw,
            )))
        }
        (InputValue::Field(field), Type::Bool) => Ok(Value::Bool(*field != FieldElement::zero())),
        (InputValue::Vec(elements), Type::Array(length, element_type)) => {
            let AbiType::Array {
                typ: element_abi, ..
            } = abi_type
            else {
                return Err(InterpretError::Type(format!(
                    "ABI type {abi_type:?} is not an array"
                )));
            };
            if elements.len() != *length as usize {
                return Err(InterpretError::Type(format!(
                    "array input has {} elements, type expects {length}",
                    elements.len()
                )));
            }
            let values = elements
                .iter()
                .map(|element| value_from_input(element, element_abi, element_type))
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(values))
        }
        (InputValue::Vec(elements), Type::Tuple(types)) => {
            let AbiType::Tuple { fields } = abi_type else {
                return Err(InterpretError::Type(format!(
                    "ABI type {abi_type:?} is not a tuple"
                )));
            };
            if elements.len() != types.len() || fields.len() != types.len() {
                return Err(InterpretError::Type(format!(
                    "tuple input has {} elements, type expects {}",
                    elements.len(),
                    types.len()
                )));
            }
            let values = fields
                .iter()
                .zip(types)
                .zip(elements)
                .map(|((field_abi, typ), element)| value_from_input(element, field_abi, typ))
                .collect::<Result<_, _>>()?;
            Ok(Value::Tuple(values))
        }
        (InputValue::Struct(map), Type::Tuple(types)) => {
            // `fields` is declaration-ordered (fields[i] matches types[i]); the input map is a
            // BTreeMap (alphabetical), so look each field up by name rather than iterate it.
            let AbiType::Struct { fields, .. } = abi_type else {
                return Err(InterpretError::Type(format!(
                    "ABI type {abi_type:?} is not a struct"
                )));
            };
            if fields.len() != types.len() {
                return Err(InterpretError::Type(format!(
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
                        InterpretError::Type(format!("missing struct field '{name}'"))
                    })?;
                    value_from_input(value, field_abi, typ)
                })
                .collect::<Result<_, _>>()?;
            Ok(Value::Tuple(values))
        }
        (InputValue::String(s), Type::String(_)) => Ok(Value::Str(s.clone())),
        (input, typ) => Err(InterpretError::Unsupported(format!(
            "input value {input:?} for parameter type {typ:?}"
        ))),
    }
}

/// The build's field modulus as a `BigInt`, for the signed-representability check.
fn field_modulus() -> BigInt {
    BigInt::from(FieldElement::modulus())
}
