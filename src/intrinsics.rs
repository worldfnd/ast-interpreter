//! Field-independent builtins the AST calls directly: slice ops, length, string<->bytes, field
//! decomposition, and the black boxes over machine words that acvm implements without a field.
//! bn254's curve and Poseidon2 black boxes live in `bn254_crypto`, behind the feature that links
//! their solver.

use acvm::BlackBoxResolutionError;
use acvm::blackbox_solver;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

use noirc_errors::Location;
use noirc_frontend::monomorphization::ast::Type;

use super::Interpreter;
use super::error::InterpretError;
use super::value::{IntValue, Value};

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
            "field_less_than" => field_less_than(&args),
            // Black boxes over machine words, with acvm's field-independent implementations.
            "sha256_compression" => sha256_compression(args),
            "keccakf1600" => keccakf1600(args),
            "blake2s" => hash_bytes(args, blackbox_solver::blake2s),
            "blake3" => hash_bytes(args, blackbox_solver::blake3),
            "aes128_encrypt" => aes128_encrypt(args),
            "ecdsa_secp256k1" => ecdsa_verify(args, blackbox_solver::ecdsa_secp256k1_verify),
            "ecdsa_secp256r1" => ecdsa_verify(args, blackbox_solver::ecdsa_secp256r1_verify),
            // Printing does not touch the value a program computes.
            "print" => Ok(Value::Unit),
            #[cfg(feature = "bn254-crypto")]
            "multi_scalar_mul"
            | "embedded_curve_add"
            | "poseidon2_permutation"
            | "derive_pedersen_generators"
                if self.field.id() == acvm::FieldId::Bn254 =>
            {
                super::bn254_crypto::call(name, args, return_type)
            }
            // bn254's crypto without its solver, comptime-only meta builtins, refcount ops.
            other => Err(InterpretError::Unsupported(format!("intrinsic '{other}'"))),
        }
    }
}

/// Move exactly `N` arguments out of the call (the AST is already type-checked, so a mismatch is
/// an interpreter bug).
pub(super) fn take<const N: usize>(args: Vec<Value>) -> Result<[Value; N], InterpretError> {
    let len = args.len();
    args.try_into().map_err(|_| {
        InterpretError::Internal(format!(
            "intrinsic arity mismatch: got {len} args, expected {N}"
        ))
    })
}

pub(super) fn into_array(value: Value) -> Result<Vec<Value>, InterpretError> {
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
    let value = field.to_bigint();
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
                    Some(Value::Str(s) | Value::LossyStr(s)) => Some(s.clone()),
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
    if field.to_bigint().bits() > bit_size {
        return Err(InterpretError::AssertionFailed {
            location,
            message: Some("call to assert_max_bit_size".to_string()),
        });
    }
    Ok(Value::Unit)
}

/// `__field_less_than(x, y)`: whether `x < y` as canonical integers in `[0, p)`.
fn field_less_than(args: &[Value]) -> Result<Value, InterpretError> {
    match args {
        [Value::Field(x), Value::Field(y)] => Ok(Value::Bool(x.as_biguint() < y.as_biguint())),
        _ => Err(InterpretError::Type(format!(
            "field_less_than expects two fields, got {args:?}"
        ))),
    }
}

/// The elements of an unsigned integer array as machine words of type `T`.
pub(super) fn words<T: TryFrom<u64>>(value: Value) -> Result<Vec<T>, InterpretError> {
    into_array(value)?
        .into_iter()
        .map(|element| match element {
            Value::Int(int) if !int.signed => int
                .value
                .to_u64()
                .and_then(|word| T::try_from(word).ok())
                .ok_or_else(|| {
                    InterpretError::Type(format!(
                        "array element {} does not fit the black box's word",
                        int.value
                    ))
                }),
            other => Err(InterpretError::Type(format!(
                "expected an unsigned integer array element, got {other:?}"
            ))),
        })
        .collect()
}

fn fixed<const N: usize, T>(values: Vec<T>, what: &str) -> Result<[T; N], InterpretError> {
    values
        .try_into()
        .map_err(|_| InterpretError::Type(format!("{what} is not {N} elements long")))
}

fn word_array(words: impl IntoIterator<Item = u64>, bits: u32) -> Value {
    Value::Array(
        words
            .into_iter()
            .map(|word| Value::Int(IntValue::canonical(false, bits, BigInt::from(word))))
            .collect(),
    )
}

/// Well-typed inputs can still violate a black box's value constraints, such as curve membership.
pub(super) fn black_box_failure(error: BlackBoxResolutionError) -> InterpretError {
    InterpretError::ValueOutOfRange(format!("black box: {error}"))
}

/// `sha256_compression(input, state)`: one compression round over `u32` words.
fn sha256_compression(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [input, state] = take(args)?;
    let input: [u32; 16] = fixed(words(input)?, "sha256 block")?;
    let mut state: [u32; 8] = fixed(words(state)?, "sha256 state")?;
    blackbox_solver::sha256_compression(&mut state, &input);
    Ok(word_array(state.into_iter().map(u64::from), 32))
}

fn keccakf1600(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [state] = take(args)?;
    let state: [u64; 25] = fixed(words(state)?, "keccak state")?;
    let state = blackbox_solver::keccakf1600(state).map_err(black_box_failure)?;
    Ok(word_array(state, 64))
}

/// A 32-byte digest of a byte array.
fn hash_bytes(
    args: Vec<Value>,
    hash: fn(&[u8]) -> Result<[u8; 32], BlackBoxResolutionError>,
) -> Result<Value, InterpretError> {
    let [input] = take(args)?;
    let digest = hash(&words::<u8>(input)?).map_err(black_box_failure)?;
    Ok(word_array(digest.into_iter().map(u64::from), 8))
}

/// `aes128_encrypt(input, iv, key)` over an input the stdlib has already padded to whole blocks.
fn aes128_encrypt(args: Vec<Value>) -> Result<Value, InterpretError> {
    let [input, iv, key] = take(args)?;
    let output = blackbox_solver::aes128_encrypt(
        &words::<u8>(input)?,
        fixed(words(iv)?, "aes iv")?,
        fixed(words(key)?, "aes key")?,
    )
    .map_err(black_box_failure)?;
    Ok(word_array(output.into_iter().map(u64::from), 8))
}

type EcdsaVerify =
    fn(&[u8; 32], &[u8; 32], &[u8; 32], &[u8; 64]) -> Result<bool, BlackBoxResolutionError>;

/// `ecdsa_*(public_key_x, public_key_y, signature, hashed_message, predicate)`: a false predicate
/// skips the check and reports the signature valid, as the ACVM does.
fn ecdsa_verify(args: Vec<Value>, verify: EcdsaVerify) -> Result<Value, InterpretError> {
    let [
        public_key_x,
        public_key_y,
        signature,
        hashed_message,
        predicate,
    ] = take(args)?;
    if !predicate.as_bool()? {
        return Ok(Value::Bool(true));
    }
    let valid = verify(
        &fixed(words(hashed_message)?, "hashed message")?,
        &fixed(words(public_key_x)?, "public key x")?,
        &fixed(words(public_key_y)?, "public key y")?,
        &fixed(words(signature)?, "signature")?,
    )
    .map_err(black_box_failure)?;
    Ok(Value::Bool(valid))
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
    use acvm::{FieldId, FieldValue};
    use std::rc::Rc;

    fn field(n: u128) -> Value {
        Value::Field(
            FieldValue::try_from_biguint(n.into(), FieldId::linked())
                .expect("the test values are below every modulus"),
        )
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
    #[cfg(feature = "bn254-crypto")]
    fn bn254_crypto_requires_bn254_runtime_field() {
        use noirc_frontend::monomorphization::ast::Program;

        let program = Program::default();
        let mut interpreter = Interpreter::new(&program, FieldId::Goldilocks);
        let zero = Value::Field(FieldValue::zero(FieldId::Goldilocks));
        let point = Value::tuple(vec![zero.clone(), zero.clone()]);
        for (name, args) in [
            (
                "embedded_curve_add",
                vec![point.clone(), point.clone(), Value::Bool(false)],
            ),
            (
                "multi_scalar_mul",
                vec![
                    Value::Array(vec![point.clone()]),
                    Value::Array(vec![point]),
                    Value::Bool(false),
                ],
            ),
            ("poseidon2_permutation", vec![Value::Array(vec![zero; 4])]),
            (
                "derive_pedersen_generators",
                vec![u8_array(b"domain"), u32v(0)],
            ),
        ] {
            assert!(
                matches!(
                    interpreter.call_intrinsic(name, args, &array_type(1), Location::dummy()),
                    Err(InterpretError::Unsupported(_))
                ),
                "{name}"
            );
        }
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
