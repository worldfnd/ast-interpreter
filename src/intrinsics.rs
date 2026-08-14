//! Pure, field-independent builtins the AST calls directly: slice ops, length, string<->bytes, and
//! field decomposition. Field-dependent black-box crypto stays `Unsupported` rather than fabricating
//! a value that could never be compared across fields.

use num_bigint::BigInt;
use num_traits::Zero;

use noirc_errors::Location;
use noirc_frontend::monomorphization::ast::Type;

use super::Interpreter;
use super::error::InterpretError;
use super::value::{IntValue, Value, field_to_bigint};

impl<'p> Interpreter<'p> {
    /// Dispatch a `#[builtin]`/`#[foreign]` call. `return_type` supplies the limb count for
    /// decomposition; `location` labels runtime assertion failures.
    pub(super) fn call_intrinsic(
        &mut self,
        name: &str,
        args: Vec<Value>,
        return_type: &Type,
        location: Location,
    ) -> Result<Value, InterpretError> {
        match name {
            "array_len" => array_len(&args),
            // Array -> slice is the identity at the AST level.
            "as_vector" => take(args).map(|[value]| value),
            "vector_push_back" => vector_push(args, true),
            "vector_push_front" => vector_push(args, false),
            "vector_pop_back" => vector_pop_back(args, location),
            "vector_pop_front" => vector_pop_front(args, location),
            "vector_insert" => vector_insert(args, location),
            "vector_remove" => vector_remove(args, location),
            "str_as_bytes" => str_as_bytes(args),
            "array_as_str_unchecked" => array_as_str_unchecked(args),
            // Builtin (attribute) names; the stdlib functions carrying them are `__to_*`.
            "to_le_radix" => to_radix(&args, false, false, return_type, location),
            "to_be_radix" => to_radix(&args, true, false, return_type, location),
            "to_le_bits" => to_radix(&args, false, true, return_type, location),
            "to_be_bits" => to_radix(&args, true, true, return_type, location),
            // True inside an unconstrained function's body (tracked across calls).
            "is_unconstrained" => Ok(Value::Bool(self.unconstrained)),
            "static_assert" => static_assert(&args, location),
            // Hints that survive to a runtime builtin call and are all field-independent no-ops:
            // `black_box` is the identity, `as_witness`/`assert_constant` return unit. (`zeroed` is
            // not here: the monomorphizer const-folds it away before we ever see the call.)
            "black_box" => take(args).map(|[value]| value),
            "as_witness" | "assert_constant" => Ok(Value::Unit),
            // `Field::assert_max_bit_size`: assert the value fits in `bit_size` bits, else fail like
            // the range constraint. Field-independent for a bound below both moduli.
            "apply_range_constraint" => apply_range_constraint(&args, location),
            // Crypto black-boxes, comptime-only meta builtins, refcount ops: a tolerated gap.
            other => Err(InterpretError::Unsupported(format!("intrinsic '{other}'"))),
        }
    }
}

/// Move exactly `N` arguments out of the call (the AST is already type-checked, so a mismatch is
/// an interpreter bug).
fn take<const N: usize>(args: Vec<Value>) -> Result<[Value; N], InterpretError> {
    let len = args.len();
    args.try_into().map_err(|_| {
        InterpretError::Internal(format!(
            "intrinsic arity mismatch: got {len} args, expected {N}"
        ))
    })
}

fn into_array(value: Value) -> Result<Vec<Value>, InterpretError> {
    match value {
        Value::Array(elements) => Ok(elements),
        other => Err(InterpretError::Type(format!(
            "expected a slice/array, got {other:?}"
        ))),
    }
}

fn array_len(args: &[Value]) -> Result<Value, InterpretError> {
    match args.first() {
        Some(Value::Array(elements)) => Ok(Value::Int(IntValue::canonical(
            false,
            32,
            BigInt::from(elements.len()),
        ))),
        Some(other) => Err(InterpretError::Type(format!(
            "array_len on a non-array {other:?}"
        ))),
        None => Err(InterpretError::Internal(
            "array_len expects one argument".to_string(),
        )),
    }
}

fn vector_push(args: Vec<Value>, back: bool) -> Result<Value, InterpretError> {
    let [array, elem] = take(args)?;
    let mut elements = into_array(array)?;
    if back {
        elements.push(elem);
    } else {
        elements.insert(0, elem);
    }
    Ok(Value::Array(elements))
}

fn vector_pop_back(args: Vec<Value>, location: Location) -> Result<Value, InterpretError> {
    let [array] = take(args)?;
    let mut elements = into_array(array)?;
    let last = elements
        .pop()
        .ok_or_else(|| InterpretError::AssertionFailed {
            location,
            message: Some(
                "Index out of bounds: vector_pop_back called on empty vector".to_string(),
            ),
        })?;
    Ok(Value::tuple(vec![Value::Array(elements), last]))
}

fn vector_pop_front(args: Vec<Value>, location: Location) -> Result<Value, InterpretError> {
    let [array] = take(args)?;
    let mut elements = into_array(array)?;
    if elements.is_empty() {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some(
                "Index out of bounds: vector_pop_front called on empty vector".to_string(),
            ),
        });
    }
    let first = elements.remove(0);
    Ok(Value::tuple(vec![first, Value::Array(elements)]))
}

fn vector_insert(args: Vec<Value>, location: Location) -> Result<Value, InterpretError> {
    let [array, index, elem] = take(args)?;
    let mut elements = into_array(array)?;
    let i = index.as_index()?;
    if i > elements.len() {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some(format!(
                "Index out of bounds: vector_insert: index {i} is out of bounds for a vector of length {}",
                elements.len()
            )),
        });
    }
    elements.insert(i, elem);
    Ok(Value::Array(elements))
}

fn vector_remove(args: Vec<Value>, location: Location) -> Result<Value, InterpretError> {
    let [array, index] = take(args)?;
    let mut elements = into_array(array)?;
    let i = index.as_index()?;
    if elements.is_empty() {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some("Index out of bounds: vector_remove called on empty vector".to_string()),
        });
    }
    if i >= elements.len() {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some(format!(
                "Index out of bounds: vector_remove: index {i} is out of bounds for a vector of length {}",
                elements.len()
            )),
        });
    }
    let removed = elements.remove(i);
    Ok(Value::tuple(vec![Value::Array(elements), removed]))
}

fn str_as_bytes(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [value] = take(args)?;
    match value {
        Value::Str(s) => Ok(Value::Array(
            s.into_bytes()
                .into_iter()
                .map(|b| Value::Int(IntValue::canonical(false, 8, BigInt::from(b))))
                .collect(),
        )),
        other => Err(InterpretError::Type(format!(
            "str_as_bytes on a non-string {other:?}"
        ))),
    }
}

fn array_as_str_unchecked(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [value] = take(args)?;
    let elements = into_array(value)?;
    let mut bytes = Vec::with_capacity(elements.len());
    for element in &elements {
        let byte = u8::try_from(element.as_index()?)
            .map_err(|_| InterpretError::Type("string byte out of range".to_string()))?;
        bytes.push(byte);
    }
    // Noir strings may be non-UTF-8; ours is a Rust `String`, so tolerate that case.
    let s = String::from_utf8(bytes).map_err(|e| {
        InterpretError::Unsupported(format!("array_as_str_unchecked on non-UTF-8 bytes: {e}"))
    })?;
    Ok(Value::Str(s))
}

/// Field decomposition into `limb_count` radix digits (faithful to Noir's `constant_to_radix`):
/// little-endian, zero-padded, big-endian reverses, over-long values error. `is_bits` = radix-2
/// `bool` limbs.
fn to_radix(
    args: &[Value],
    big_endian: bool,
    is_bits: bool,
    return_type: &Type,
    location: Location,
) -> Result<Value, InterpretError> {
    let field = match args.first() {
        Some(Value::Field(f)) => f,
        Some(other) => {
            return Err(InterpretError::Type(format!(
                "decomposition of a non-field {other:?}"
            )));
        }
        None => {
            return Err(InterpretError::Internal(
                "decomposition expects a field argument".to_string(),
            ));
        }
    };
    let radix: u32 = if is_bits {
        2
    } else {
        match args.get(1) {
            Some(value) => u32::try_from(value.as_index()?)
                .map_err(|_| InterpretError::Type("radix out of range".to_string()))?,
            None => {
                return Err(InterpretError::Internal(
                    "radix decomposition expects a radix argument".to_string(),
                ));
            }
        }
    };
    let limb_count = match return_type {
        Type::Array(len, _) => *len,
        other => {
            return Err(InterpretError::Type(format!(
                "decomposition return type is not an array: {other:?}"
            )));
        }
    };
    if !(2..=256).contains(&radix) {
        return Err(InterpretError::Type(format!(
            "radix {radix} must be in [2, 256]"
        )));
    }
    let value = field_to_bigint(field);
    // `to_radix_le` represents zero as a single `[0]` limb; treat zero as no significant limbs.
    let digits: Vec<u8> = if value.is_zero() {
        Vec::new()
    } else {
        value.to_radix_le(radix).1
    };
    if (limb_count as usize) < digits.len() {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some(format!(
                "Field failed to decompose into specified {limb_count} limbs"
            )),
        });
    }
    let mut limbs: Vec<Value> = (0..limb_count as usize)
        .map(|i| {
            let digit = digits.get(i).copied().unwrap_or(0);
            if is_bits {
                Value::Bool(digit != 0)
            } else {
                Value::Int(IntValue::canonical(false, 8, BigInt::from(digit)))
            }
        })
        .collect();
    if big_endian {
        limbs.reverse();
    }
    Ok(Value::Array(limbs))
}

/// `static_assert(condition, message, …)`: kept as a runtime builtin, its condition may fold to a
/// field-dependent value (e.g. the decomposition modulus guard).
fn static_assert(args: &[Value], location: Location) -> Result<Value, InterpretError> {
    match args.first() {
        Some(condition) => {
            if condition.as_bool()? {
                Ok(Value::Unit)
            } else {
                let message = match args.get(1) {
                    Some(Value::Str(s)) => Some(s.clone()),
                    _ => None,
                };
                Err(InterpretError::AssertionFailed { location, message })
            }
        }
        None => Err(InterpretError::Internal(
            "static_assert expects a condition".to_string(),
        )),
    }
}

/// `apply_range_constraint(value, bit_size)` (from `Field::assert_max_bit_size`): the value's
/// canonical integer must fit in `bit_size` bits. For a bound below both moduli this is
/// field-independent; over-size is a failed range constraint, exactly as the ACVM executor treats it.
fn apply_range_constraint(args: &[Value], location: Location) -> Result<Value, InterpretError> {
    let field = match args.first() {
        Some(Value::Field(f)) => f,
        Some(other) => {
            return Err(InterpretError::Type(format!(
                "apply_range_constraint on a non-field {other:?}"
            )));
        }
        None => {
            return Err(InterpretError::Internal(
                "apply_range_constraint expects a value".to_string(),
            ));
        }
    };
    let bit_size = arg_u64(args, 1)?;
    if field_to_bigint(field).bits() > bit_size {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some("call to assert_max_bit_size".to_string()),
        });
    }
    Ok(Value::Unit)
}

fn arg_u64(args: &[Value], i: usize) -> Result<u64, InterpretError> {
    let value = args
        .get(i)
        .ok_or_else(|| InterpretError::Internal(format!("intrinsic missing argument {i}")))?;
    let (_, digits) = value.as_int()?.unsigned_repr().to_u64_digits();
    match digits.as_slice() {
        [] => Ok(0),
        [d] => Ok(*d),
        _ => Err(InterpretError::Type("value exceeds u64".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acvm::FieldElement;
    use std::rc::Rc;

    fn field(n: u128) -> Value {
        Value::Field(FieldElement::from(n))
    }
    fn u32v(n: u32) -> Value {
        Value::Int(IntValue::canonical(false, 32, BigInt::from(n)))
    }
    fn u8_array(bytes: &[u8]) -> Value {
        Value::Array(
            bytes
                .iter()
                .map(|b| Value::Int(IntValue::canonical(false, 8, BigInt::from(*b))))
                .collect(),
        )
    }
    fn array_type(len: u32) -> Type {
        Type::Array(len, Rc::new(Type::Field)) // to_radix only reads the length
    }

    #[test]
    fn to_le_radix_decomposes_little_endian_bytes() {
        // 258 = 0x0102 -> [2, 1, 0, 0]
        let out = to_radix(
            &[field(258), u32v(256)],
            false,
            false,
            &array_type(4),
            Location::dummy(),
        )
        .unwrap();
        assert_eq!(out, u8_array(&[2, 1, 0, 0]));
    }

    #[test]
    fn to_be_radix_reverses_the_digits() {
        let out = to_radix(
            &[field(258), u32v(256)],
            true,
            false,
            &array_type(4),
            Location::dummy(),
        )
        .unwrap();
        assert_eq!(out, u8_array(&[0, 0, 1, 2]));
    }

    #[test]
    fn to_le_bits_sets_the_right_bits() {
        // 258 = 0b1_0000_0010 -> bit 1 and bit 8 set.
        let out = to_radix(
            &[field(258)],
            false,
            true,
            &array_type(10),
            Location::dummy(),
        )
        .unwrap();
        let mut expected = vec![false; 10];
        expected[1] = true;
        expected[8] = true;
        assert_eq!(
            out,
            Value::Array(expected.into_iter().map(Value::Bool).collect())
        );
    }

    #[test]
    fn zero_decomposes_to_all_zero_limbs() {
        let out = to_radix(
            &[field(0), u32v(256)],
            false,
            false,
            &array_type(3),
            Location::dummy(),
        )
        .unwrap();
        assert_eq!(out, u8_array(&[0, 0, 0]));
    }

    #[test]
    fn unconstrained_radix_three_is_supported() {
        let out = to_radix(
            &[field(11), u32v(3)],
            false,
            false,
            &array_type(4),
            Location::dummy(),
        )
        .unwrap();
        assert_eq!(out, u8_array(&[2, 0, 1, 0]));
    }

    #[test]
    fn decomposition_errors_when_limbs_too_few() {
        // 258 needs two bytes; one limb cannot hold it.
        assert!(matches!(
            to_radix(
                &[field(258), u32v(256)],
                false,
                false,
                &array_type(1),
                Location::dummy(),
            ),
            Err(InterpretError::AssertionFailed {
                message: Some(message),
                ..
            }) if message == "Field failed to decompose into specified 1 limbs"
        ));
    }

    #[test]
    fn vector_bounds_errors_match_noir() {
        type VectorOp = fn(Vec<Value>, Location) -> Result<Value, InterpretError>;
        let array = || Value::Array(vec![u32v(1), u32v(2)]);
        let cases: [(VectorOp, Vec<Value>, &str); 5] = [
            (
                vector_pop_back,
                vec![Value::Array(vec![])],
                "Index out of bounds: vector_pop_back called on empty vector",
            ),
            (
                vector_pop_front,
                vec![Value::Array(vec![])],
                "Index out of bounds: vector_pop_front called on empty vector",
            ),
            (
                vector_insert,
                vec![array(), u32v(3), u32v(9)],
                "Index out of bounds: vector_insert: index 3 is out of bounds for a vector of length 2",
            ),
            (
                vector_remove,
                vec![Value::Array(vec![]), u32v(0)],
                "Index out of bounds: vector_remove called on empty vector",
            ),
            (
                vector_remove,
                vec![array(), u32v(2)],
                "Index out of bounds: vector_remove: index 2 is out of bounds for a vector of length 2",
            ),
        ];
        for (operation, args, expected) in cases {
            match operation(args, Location::dummy()) {
                Err(InterpretError::AssertionFailed {
                    message: Some(message),
                    ..
                }) => assert_eq!(message, expected),
                other => panic!("expected AssertionFailed, got {other:?}"),
            }
        }
    }

    #[test]
    fn range_constraint_accepts_fitting_and_rejects_oversize() {
        let loc = Location::dummy();
        assert_eq!(
            apply_range_constraint(&[field(255), u32v(8)], loc).unwrap(),
            Value::Unit
        );
        assert!(matches!(
            apply_range_constraint(&[field(256), u32v(8)], loc),
            Err(InterpretError::AssertionFailed { .. })
        ));
        assert_eq!(
            apply_range_constraint(&[field(65535), u32v(16)], loc).unwrap(),
            Value::Unit
        );
        assert!(matches!(
            apply_range_constraint(&[field(65536), u32v(16)], loc),
            Err(InterpretError::AssertionFailed { .. })
        ));
        assert_eq!(
            apply_range_constraint(&[field(0), u32v(0)], loc).unwrap(),
            Value::Unit
        );
        assert!(matches!(
            apply_range_constraint(&[field(1), u32v(0)], loc),
            Err(InterpretError::AssertionFailed { .. })
        ));
    }
}
