//! Property-based fuzzer for the crate's **field-agnostic value semantics**.
//!
//! These properties cover checked/wrapping arithmetic, casts, shifts, division/modulo, and
//! integer-to-field encoding across supported fields.
//!
//! Scope is value logic only: no ASTs, interpreter, or Noir project compilation.

use acvm::{AcirField, FieldElement};
use num_bigint::BigInt;
use num_traits::{One, Zero};
use proptest::prelude::*;

use noirc_frontend::ast::BinaryOpKind;

use crate::error::InterpretError;
use crate::eval::eval_int_binary;
use crate::value::{IntValue, Value, field_to_bigint, wrap};

/// The integer widths this crate supports (bit sizes of Noir's `u*`/`i*` types).
fn width() -> impl Strategy<Value = u8> {
    prop_oneof![Just(8u8), Just(16u8), Just(32u8), Just(64u8), Just(128u8),]
}

/// The smallest supported width strictly greater than `bits` (saturating at the 128-bit max). Used
/// so the cast round-trip exercises genuine widening instead of collapsing to a same-width no-op.
fn wider_than(bits: u8) -> u8 {
    match bits {
        8 => 16,
        16 => 32,
        32 => 64,
        _ => 128,
    }
}

/// An integer *type*: signedness paired with one of the supported widths.
fn int_type() -> impl Strategy<Value = (bool, u8)> {
    (any::<bool>(), width())
}

/// An in-range [`IntValue`] of the given type: draw a magnitude and a sign, then let
/// [`IntValue::canonical`] wrap the raw value into the type's range (two's complement).
fn int_value_of(signed: bool, bits: u8) -> impl Strategy<Value = IntValue> {
    (any::<u128>(), any::<bool>()).prop_map(move |(mag, neg)| {
        let raw = if neg {
            -BigInt::from(mag)
        } else {
            BigInt::from(mag)
        };
        IntValue::canonical(signed, bits, raw)
    })
}

/// A pair of values sharing ONE integer type — the shape `eval_int_binary` requires (both operands,
/// including a shift amount, are the same `(signed, bits)`).
fn same_typed_pair() -> impl Strategy<Value = (IntValue, IntValue)> {
    int_type()
        .prop_flat_map(|(signed, bits)| (int_value_of(signed, bits), int_value_of(signed, bits)))
}

proptest! {
    /// P1 — checked Add/Sub/Mul errors exactly outside the type range.
    #[test]
    fn p1_checked_matches_range(
        (a, b) in same_typed_pair(),
        op in prop_oneof![
            Just(BinaryOpKind::Add),
            Just(BinaryOpKind::Subtract),
            Just(BinaryOpKind::Multiply),
        ],
    ) {
        let signed = a.signed;
        let bits = a.bits;
        let math = match op {
            BinaryOpKind::Add => &a.value + &b.value,
            BinaryOpKind::Subtract => &a.value - &b.value,
            BinaryOpKind::Multiply => &a.value * &b.value,
            _ => unreachable!(),
        };
        let (min, max) = IntValue::range(signed, bits);
        let result = eval_int_binary(op, a, b);

        if math < min || math > max {
            prop_assert!(
                matches!(result, Err(InterpretError::Overflow(_))),
                "expected overflow for exact value {math}, got {result:?}"
            );
        } else {
            prop_assert!(
                matches!(result, Ok(Value::Int(_))),
                "expected Ok(Int) for {math}, got {result:?}"
            );
            let Ok(Value::Int(iv)) = result else { unreachable!() };
            prop_assert_eq!(&iv.value, &math);
            prop_assert_eq!(iv.value, wrap(signed, bits, math));
        }
    }

    /// P2 — signedness flips and widen/narrow casts preserve the expected value.
    #[test]
    fn p2_cast_roundtrips(
        (signed, bits) in int_type(),
        (mag, neg) in (any::<u128>(), any::<bool>()),
    ) {
        let raw = if neg { -BigInt::from(mag) } else { BigInt::from(mag) };
        let v = IntValue::canonical(signed, bits, raw);

        // (a) flip signedness at the same width, then flip back.
        let flip = IntValue::canonical(!signed, bits, v.value.clone());
        let back = IntValue::canonical(signed, bits, flip.value.clone());
        prop_assert_eq!(&back.value, &v.value);
        prop_assert_eq!(flip.unsigned_repr(), v.unsigned_repr());

        // (b) widen to the next larger width (genuine widening for bits < 128), then narrow back.
        let bits2 = wider_than(bits);
        let wide = IntValue::canonical(signed, bits2, v.value.clone());
        prop_assert_eq!(&wide.value, &v.value);
        let narrow = IntValue::canonical(signed, bits, wide.value.clone());
        prop_assert_eq!(&narrow.value, &v.value);
    }

    /// P3(a) — shifting by `amount >= bits` overflows in both directions.
    #[test]
    fn p3a_over_shift_errors(
        bits in width(),
        (mag, neg) in (any::<u128>(), any::<bool>()),
        extra in 0u32..=64,
    ) {
        let raw = if neg { -BigInt::from(mag) } else { BigInt::from(mag) };
        let a = IntValue::canonical(false, bits, raw);
        let amount = IntValue::canonical(false, bits, BigInt::from(bits as u32 + extra));

        let shl = eval_int_binary(BinaryOpKind::ShiftLeft, a.clone(), amount.clone());
        let shr = eval_int_binary(BinaryOpKind::ShiftRight, a, amount);
        prop_assert!(matches!(shl, Err(InterpretError::Overflow(_))), "shl: {shl:?}");
        prop_assert!(matches!(shr, Err(InterpretError::Overflow(_))), "shr: {shr:?}");
    }

    /// P3(b) — in-range shifts match independent arithmetic references.
    #[test]
    fn p3b_in_range_shifts(
        (signed, bits) in int_type(),
        (mag, neg) in (any::<u128>(), any::<bool>()),
        amt_seed in any::<u8>(),
    ) {
        let raw = if neg { -BigInt::from(mag) } else { BigInt::from(mag) };
        let a = IntValue::canonical(signed, bits, raw);
        let amount = usize::from(amt_seed % bits);
        let b = IntValue::canonical(signed, bits, BigInt::from(amount));

        // Capture independent references BEFORE the operands are moved into `eval_int_binary`.
        let a_repr = a.unsigned_repr();
        let a_value = a.value.clone();

        let shl = eval_int_binary(BinaryOpKind::ShiftLeft, a.clone(), b.clone());
        let shr = eval_int_binary(BinaryOpKind::ShiftRight, a, b);
        prop_assert!(matches!(shl, Ok(Value::Int(_))), "shl: {shl:?}");
        prop_assert!(matches!(shr, Ok(Value::Int(_))), "shr: {shr:?}");
        let Ok(Value::Int(shl_iv)) = shl else { unreachable!() };
        let Ok(Value::Int(shr_iv)) = shr else { unreachable!() };

        // Left shift keeps the low `bits` bits.
        let two_pow_bits = BigInt::one() << (bits as usize);
        prop_assert_eq!(shl_iv.unsigned_repr(), (a_repr << amount) % &two_pow_bits);

        // Right shift equals floor(a / 2^amount).
        let two_pow = BigInt::one() << amount;
        let lo = &shr_iv.value * &two_pow;
        let hi = (&shr_iv.value + BigInt::one()) * &two_pow;
        prop_assert!(
            lo <= a_value && a_value < hi,
            "shr not floor(a / 2^amount): shr={shr_iv:?} amount={amount}"
        );
    }

    /// P4 — integer-to-field encoding reduces the bit pattern modulo the active field.
    #[test]
    fn p4_field_roundtrip(
        (signed, bits) in int_type(),
        (mag, neg) in (any::<u128>(), any::<bool>()),
    ) {
        let raw = if neg { -BigInt::from(mag) } else { BigInt::from(mag) };
        let iv = IntValue::canonical(signed, bits, raw);

        let modulus = BigInt::from(FieldElement::modulus());
        let repr = iv.unsigned_repr();
        let field_back = field_to_bigint(&iv.to_field());

        // Always: the field reduces the bit pattern mod the field modulus.
        prop_assert_eq!(&field_back, &(&repr % &modulus));
        // Exact identity only when the value fits below the modulus.
        if repr < modulus {
            prop_assert_eq!(field_back, repr);
        }
    }

    /// P5 — div/mod handle zero, signed MIN/-1, and `q * b + r == a`.
    #[test]
    fn p5_div_mod_law((a, b) in same_typed_pair()) {
        let signed = a.signed;
        let bits = a.bits;
        let (min, _) = IntValue::range(signed, bits);

        let div = eval_int_binary(BinaryOpKind::Divide, a.clone(), b.clone());
        let rem = eval_int_binary(BinaryOpKind::Modulo, a.clone(), b.clone());

        if b.value.is_zero() {
            prop_assert!(matches!(div, Err(InterpretError::DivisionByZero)), "div: {div:?}");
            prop_assert!(matches!(rem, Err(InterpretError::DivisionByZero)), "rem: {rem:?}");
        } else if signed && a.value == min && b.value == -BigInt::from(1) {
            prop_assert!(matches!(div, Err(InterpretError::Overflow(_))), "div: {div:?}");
            prop_assert!(matches!(rem, Err(InterpretError::Overflow(_))), "rem: {rem:?}");
        } else {
            prop_assert!(matches!(div, Ok(Value::Int(_))), "div: {div:?}");
            prop_assert!(matches!(rem, Ok(Value::Int(_))), "rem: {rem:?}");
            let Ok(Value::Int(q)) = div else { unreachable!() };
            let Ok(Value::Int(r)) = rem else { unreachable!() };
            prop_assert_eq!(&q.value * &b.value + &r.value, a.value);
        }
    }
}

// Deterministic coverage for P5's rare error paths.

/// Division/modulo by a zero divisor is `DivisionByZero` for every signedness/width.
#[test]
fn div_and_mod_by_zero_error() {
    for (signed, bits) in [(false, 8u8), (true, 8), (false, 64), (true, 128)] {
        let a = IntValue::canonical(signed, bits, BigInt::from(7));
        let zero = IntValue::canonical(signed, bits, BigInt::from(0));
        assert!(matches!(
            eval_int_binary(BinaryOpKind::Divide, a.clone(), zero.clone()),
            Err(InterpretError::DivisionByZero)
        ));
        assert!(matches!(
            eval_int_binary(BinaryOpKind::Modulo, a, zero),
            Err(InterpretError::DivisionByZero)
        ));
    }
}

/// Signed `MIN / -1` and `MIN % -1` overflow (the quotient `2^(bits-1)` leaves the signed range),
/// matching Rust's checked `div`/`rem`.
#[test]
fn signed_min_div_mod_neg_one_overflow() {
    for bits in [8u8, 16, 32, 64, 128] {
        let (min, _) = IntValue::range(true, bits);
        let a = IntValue::canonical(true, bits, min);
        let neg_one = IntValue::canonical(true, bits, -BigInt::from(1));
        assert!(matches!(
            eval_int_binary(BinaryOpKind::Divide, a.clone(), neg_one.clone()),
            Err(InterpretError::Overflow(_))
        ));
        assert!(matches!(
            eval_int_binary(BinaryOpKind::Modulo, a, neg_one),
            Err(InterpretError::Overflow(_))
        ));
    }
}
