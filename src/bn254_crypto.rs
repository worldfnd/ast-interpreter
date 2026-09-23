//! bn254's curve, Pedersen-generator and Poseidon2 black boxes through `bn254_blackbox_solver`.
//! They run only for bn254 programs and hold bn254 elements whatever field the build links.

use acvm::{AcirField, Bn254FieldElement, FieldValue};
use noirc_frontend::monomorphization::ast::Type;

use super::error::InterpretError;
use super::intrinsics::{black_box_failure, into_array, take, words};
use super::value::Value;

pub(crate) fn call(
    name: &str,
    args: Vec<Value>,
    return_type: &Type,
) -> Result<Value, InterpretError> {
    match name {
        "multi_scalar_mul" => multi_scalar_mul(args),
        "embedded_curve_add" => embedded_curve_add(args),
        "poseidon2_permutation" => poseidon2_permutation(args),
        "derive_pedersen_generators" => derive_pedersen_generators(args, return_type),
        other => Err(InterpretError::Internal(format!(
            "{other} is not a bn254 black box"
        ))),
    }
}

fn element(value: &Value) -> Result<Bn254FieldElement, InterpretError> {
    match value {
        Value::Field(field) => field
            .to_bn254_element()
            .ok_or_else(|| InterpretError::Type(format!("{field:?} is not a bn254 element"))),
        other => Err(InterpretError::Type(format!(
            "expected a Field, got {other:?}"
        ))),
    }
}

fn field(element: Bn254FieldElement) -> Value {
    Value::Field(FieldValue::from_bn254_element(element))
}

/// An `EmbeddedCurvePoint` or an `EmbeddedCurveScalar`: a struct of two fields.
fn pair(value: &Value) -> Result<(Bn254FieldElement, Bn254FieldElement), InterpretError> {
    Ok((
        element(&value.tuple_field(0)?)?,
        element(&value.tuple_field(1)?)?,
    ))
}

fn pairs(value: Value) -> Result<Vec<(Bn254FieldElement, Bn254FieldElement)>, InterpretError> {
    into_array(value)?.iter().map(pair).collect()
}

fn point((x, y): (Bn254FieldElement, Bn254FieldElement)) -> Value {
    Value::tuple(vec![field(x), field(y)])
}

/// `multi_scalar_mul(points, scalars, predicate) -> [EmbeddedCurvePoint; 1]`; a false predicate
/// yields the zero point, as the solver does.
fn multi_scalar_mul(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [points, scalars, predicate] = take(args)?;
    let result = if predicate.as_bool()? {
        let points: Vec<_> = pairs(points)?
            .into_iter()
            .flat_map(|(x, y)| [x, y])
            .collect();
        let (lo, hi): (Vec<_>, Vec<_>) = pairs(scalars)?.into_iter().unzip();
        bn254_blackbox_solver::multi_scalar_mul(&points, &lo, &hi).map_err(black_box_failure)?
    } else {
        (Bn254FieldElement::zero(), Bn254FieldElement::zero())
    };
    Ok(Value::Array(vec![point(result)]))
}

/// `embedded_curve_add(point1, point2, predicate) -> [EmbeddedCurvePoint; 1]`.
fn embedded_curve_add(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [point1, point2, predicate] = take(args)?;
    let result = if predicate.as_bool()? {
        let (x1, y1) = pair(&point1)?;
        let (x2, y2) = pair(&point2)?;
        bn254_blackbox_solver::embedded_curve_add([x1, y1], [x2, y2]).map_err(black_box_failure)?
    } else {
        (Bn254FieldElement::zero(), Bn254FieldElement::zero())
    };
    Ok(Value::Array(vec![point(result)]))
}

fn poseidon2_permutation(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [input] = take(args)?;
    let inputs: Vec<_> = into_array(input)?
        .iter()
        .map(element)
        .collect::<Result<_, _>>()?;
    let state = bn254_blackbox_solver::poseidon2_permutation(&inputs).map_err(black_box_failure)?;
    Ok(Value::Array(state.into_iter().map(field).collect()))
}

/// `derive_pedersen_generators(domain_separator_bytes, starting_index) -> [EmbeddedCurvePoint; N]`,
/// with `N` read off the return type.
fn derive_pedersen_generators(
    args: Vec<Value>,
    return_type: &Type,
) -> Result<Value, InterpretError> {
    let [domain_separator, starting_index] = take(args)?;
    let count = match return_type {
        Type::Array(length, _) => *length,
        other => {
            return Err(InterpretError::Type(format!(
                "generators return type is not an array: {other:?}"
            )));
        }
    };
    let starting_index = u32::try_from(starting_index.as_index()?)
        .map_err(|_| InterpretError::Type("generator index out of range".to_string()))?;
    let generators = bn254_blackbox_solver::derive_generators(
        &words::<u8>(domain_separator)?,
        count,
        starting_index,
    );
    Ok(Value::Array(
        generators
            .into_iter()
            .map(|generator| {
                point((
                    Bn254FieldElement::from_repr(generator.x),
                    Bn254FieldElement::from_repr(generator.y),
                ))
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_curve_inputs_fail_only_when_predicate_is_true() {
        let zero = Bn254FieldElement::zero();
        let one = Bn254FieldElement::one();
        for (name, operands, reason) in [
            (
                "embedded_curve_add",
                vec![point((one, one)), point((zero, zero))],
                "not on curve",
            ),
            (
                "multi_scalar_mul",
                vec![
                    Value::Array(vec![point((one, one))]),
                    Value::Array(vec![point((one, zero))]),
                ],
                "not on curve",
            ),
            (
                "multi_scalar_mul",
                vec![
                    Value::Array(vec![point((zero, zero))]),
                    Value::Array(vec![point((-one, zero))]),
                ],
                "not less than 2^128",
            ),
        ] {
            for predicate in [false, true] {
                let mut args = operands.clone();
                args.push(Value::Bool(predicate));
                let result = call(name, args, &Type::Unit);
                if predicate {
                    assert!(
                        matches!(result, Err(InterpretError::ValueOutOfRange(message)) if message.contains(reason)),
                        "{name}: {reason}"
                    );
                } else {
                    assert_eq!(result.unwrap(), Value::Array(vec![point((zero, zero))]));
                }
            }
        }
    }
}
