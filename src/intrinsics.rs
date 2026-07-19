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
    /// decomposition; `location` labels a failing `static_assert`.
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
            "vector_pop_back" => vector_pop_back(args),
            "vector_pop_front" => vector_pop_front(args),
            "vector_insert" => vector_insert(args),
            "vector_remove" => vector_remove(args),
            "str_as_bytes" => str_as_bytes(args),
            "array_as_str_unchecked" => array_as_str_unchecked(args),
            // Builtin (attribute) names; the stdlib functions carrying them are `__to_*`.
            "to_le_radix" => to_radix(&args, false, false, return_type),
            "to_be_radix" => to_radix(&args, true, false, return_type),
            "to_le_bits" => to_radix(&args, false, true, return_type),
            "to_be_bits" => to_radix(&args, true, true, return_type),
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
            // Field-independent pure-integer foreign builtins. `unsafe_cast` is a truncating
            // Field->int cast (the same operation as an `as` cast).
            "unsafe_cast" => {
                let [value] = take(args)?;
                self.eval_cast(value, return_type)
            }
            // A u32<->u64 bit-interleave / de-interleave; the bit-width argument does not affect the
            // runtime result.
            "spread_inner" => spread_inner(&args),
            "unspread_inner" => unspread_inner(&args),
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

/// `push_back`/`push_front`: append (or prepend) an element, returning the new slice.
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

/// `pop_back`: return `(remaining_slice, last)`.
fn vector_pop_back(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [array] = take(args)?;
    let mut elements = into_array(array)?;
    let last = elements
        .pop()
        .ok_or_else(|| InterpretError::Type("pop_back on an empty slice".to_string()))?;
    Ok(Value::tuple(vec![Value::Array(elements), last]))
}

/// `pop_front`: return `(first, remaining_slice)`.
fn vector_pop_front(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [array] = take(args)?;
    let mut elements = into_array(array)?;
    if elements.is_empty() {
        return Err(InterpretError::Type(
            "pop_front on an empty slice".to_string(),
        ));
    }
    let first = elements.remove(0);
    Ok(Value::tuple(vec![first, Value::Array(elements)]))
}

/// `insert(i, elem)`: shift elements from `i` right and place `elem`.
fn vector_insert(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [array, index, elem] = take(args)?;
    let mut elements = into_array(array)?;
    let i = index.as_index()?;
    if i > elements.len() {
        return Err(InterpretError::Type(format!(
            "insert index {i} out of bounds (len {})",
            elements.len()
        )));
    }
    elements.insert(i, elem);
    Ok(Value::Array(elements))
}

/// `remove(i)`: return `(slice_without_i, removed)`.
fn vector_remove(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [array, index] = take(args)?;
    let mut elements = into_array(array)?;
    let i = index.as_index()?;
    if i >= elements.len() {
        return Err(InterpretError::Type(format!(
            "remove index {i} out of bounds (len {})",
            elements.len()
        )));
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
    if !(2..=256).contains(&radix) || (radix & (radix - 1)) != 0 {
        return Err(InterpretError::Type(format!(
            "radix {radix} must be a power of two in [2, 256]"
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
        return Err(InterpretError::Type(format!(
            "field does not fit in {limb_count} limbs"
        )));
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

/// `spread_inner(value: u32, bits: u32) -> u64`: interleave zero bits between `value`'s bits. The
/// `bits` argument does not affect the runtime result.
fn spread_inner(args: &[Value]) -> Result<Value, InterpretError> {
    let value = arg_u64(args, 0)? as u32;
    Ok(Value::Int(IntValue::canonical(
        false,
        64,
        BigInt::from(spread_bits(value)),
    )))
}

/// `unspread_inner(value: u64, bits: u32) -> (u32, u32)`: split a spread sum back into its `(odd,
/// even)` lanes. `bits` does not affect the runtime result.
fn unspread_inner(args: &[Value]) -> Result<Value, InterpretError> {
    let value = arg_u64(args, 0)?;
    let (odd, even) = unspread_bits(value);
    Ok(Value::tuple(vec![
        Value::Int(IntValue::canonical(false, 32, BigInt::from(odd))),
        Value::Int(IntValue::canonical(false, 32, BigInt::from(even))),
    ]))
}

/// Read `args[i]` as an unsigned integer that fits in a `u64`.
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

// Pure integer bit-spreading — no field, so it is identical under both fields.

/// Interleave zero bits between each of a u32's bits (bit `i` -> bit `2i`).
fn spread_bits(v: u32) -> u64 {
    let mut x = v as u64;
    x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    x = (x | (x << 1)) & 0x5555_5555_5555_5555;
    x
}

/// Gather the even-positioned bits of a u64 into contiguous low bits.
fn compact_bits(mut x: u64) -> u32 {
    x &= 0x5555_5555_5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333_3333_3333;
    x = (x | (x >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x >> 4)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x >> 8)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x >> 16)) & 0x0000_0000_FFFF_FFFF;
    x as u32
}

/// Extract the odd and even lanes of a spread sum. Returns `(odd, even)` — odd at index 0.
fn unspread_bits(v: u64) -> (u32, u32) {
    let even = compact_bits(v);
    let odd = compact_bits(v >> 1);
    (odd, even)
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
        let out = to_radix(&[field(258), u32v(256)], false, false, &array_type(4)).unwrap();
        assert_eq!(out, u8_array(&[2, 1, 0, 0]));
    }

    #[test]
    fn to_be_radix_reverses_the_digits() {
        let out = to_radix(&[field(258), u32v(256)], true, false, &array_type(4)).unwrap();
        assert_eq!(out, u8_array(&[0, 0, 1, 2]));
    }

    #[test]
    fn to_le_bits_sets_the_right_bits() {
        // 258 = 0b1_0000_0010 -> bit 1 and bit 8 set.
        let out = to_radix(&[field(258)], false, true, &array_type(10)).unwrap();
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
        let out = to_radix(&[field(0), u32v(256)], false, false, &array_type(3)).unwrap();
        assert_eq!(out, u8_array(&[0, 0, 0]));
    }

    #[test]
    fn decomposition_errors_when_limbs_too_few() {
        // 258 needs two bytes; one limb cannot hold it.
        assert!(matches!(
            to_radix(&[field(258), u32v(256)], false, false, &array_type(1)),
            Err(InterpretError::Type(_))
        ));
    }

    #[test]
    fn pop_back_on_empty_errors() {
        assert!(matches!(
            vector_pop_back(vec![Value::Array(vec![])]),
            Err(InterpretError::Type(_))
        ));
    }

    #[test]
    fn insert_past_the_end_errors() {
        let arr = Value::Array(vec![u32v(1), u32v(2)]);
        assert!(matches!(
            vector_insert(vec![arr, u32v(3), u32v(9)]),
            Err(InterpretError::Type(_))
        ));
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

    #[test]
    fn spread_bits_interleaves_zeros() {
        assert_eq!(spread_bits(0b101), 0x11);
        assert_eq!(spread_bits(0b111), 0x15);
        assert_eq!(spread_bits(0xFFFF), 0x5555_5555);
        assert_eq!(spread_bits(0), 0);
        assert_eq!(spread_bits(1), 1);
        assert_eq!(spread_bits(0xFFFF_FFFF), 0x5555_5555_5555_5555);
    }

    #[test]
    fn unspread_bits_splits_odd_even_lanes() {
        assert_eq!(unspread_bits(0x5555_5555), (0, 0xFFFF));
        assert_eq!(unspread_bits(17), (0, 5));
        assert_eq!(unspread_bits(27), (3, 5));
    }

    #[test]
    fn spread_unspread_round_trip_and_lane_order() {
        for v in [5u32, 7, 0xFFFF, 0xABCD] {
            assert_eq!(unspread_bits(spread_bits(v)), (0, v));
        }
        // Pack even lane 5 and odd lane 3; unspread recovers (odd=3, even=5).
        let mixed = spread_bits(5) | (spread_bits(3) << 1);
        assert_eq!(mixed, 27);
        assert_eq!(unspread_bits(mixed), (3, 5));
    }

    #[test]
    fn spread_intrinsic_returns_u64_and_unspread_returns_tuple() {
        assert_eq!(
            spread_inner(&[u32v(5), u32v(2)]).unwrap(),
            Value::Int(IntValue::canonical(false, 64, BigInt::from(0x11u64)))
        );
        // args[1] (bits) does not affect the result.
        assert_eq!(
            spread_inner(&[u32v(5), u32v(999)]).unwrap(),
            Value::Int(IntValue::canonical(false, 64, BigInt::from(0x11u64)))
        );
        let u64v = |n: u64| Value::Int(IntValue::canonical(false, 64, BigInt::from(n)));
        assert_eq!(
            unspread_inner(&[u64v(27), u32v(2)]).unwrap(),
            Value::tuple(vec![u32v(3), u32v(5)])
        );
    }
}
