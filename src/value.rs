use std::cell::RefCell;
use std::rc::Rc;

use acvm::{AcirField, FieldElement};
use num_bigint::{BigInt, Sign};
use num_traits::{One, Zero};

use noirc_frontend::monomorphization::ast::FuncId;

use super::error::InterpretError;

/// A runtime value produced while interpreting the monomorphized AST.
///
/// Integers carry an explicit width, signedness, and a `BigInt` of the canonical mathematical value
/// (not a field-reduced one), so a `u64` at or above the field modulus survives intact and the same
/// computation yields identical integer/bool results under bn254 and Goldilocks — the property the
/// cross-field differential checks. `Field` values use the compiled-in [`FieldElement`].
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Field(FieldElement),
    Int(IntValue),
    Bool(bool),
    Unit,
    Array(Vec<Value>),
    // Fields are shared cells so `&mut s.field` aliases; value reads `deep_copy` out.
    Tuple(Vec<Rc<RefCell<Value>>>),
    Str(String),
    Function(FuncId),
    // Shared cell. `auto_deref` = a `let mut`/mutable-param slot (loaded on a bare read); a plain
    // `&`/`&mut` reference is `false`.
    Ref(Rc<RefCell<Value>>, bool),
}

/// A machine integer: width, signedness, and the canonical value.
///
/// Invariant: `value` is the mathematical integer in the type's range — unsigned in
/// `[0, 2^bits)`, signed in `[-2^(bits-1), 2^(bits-1))`.
#[derive(Clone, Debug, PartialEq)]
pub struct IntValue {
    pub signed: bool,
    pub bits: u8,
    pub value: BigInt,
}

fn pow2(bits: u8) -> BigInt {
    BigInt::one() << bits as usize
}

/// The non-negative integer value of a field element. Unlike `FieldElement::to_u128`, this does
/// not panic for values `>= 2^128` (bn254 elements are up to ~254 bits).
pub fn field_to_bigint(field: &FieldElement) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, &field.to_be_bytes())
}

/// Reduce `raw` into the canonical two's-complement representative for `(signed, bits)`.
pub fn wrap(signed: bool, bits: u8, raw: BigInt) -> BigInt {
    let modulus = pow2(bits);
    let mut u = raw % &modulus;
    if u.sign() == Sign::Minus {
        u += &modulus;
    }
    if signed && u >= pow2(bits - 1) {
        u -= &modulus;
    }
    u
}

impl IntValue {
    /// Construct by *wrapping* `raw` into the type's range (two's complement) — the truncating
    /// constructor used for casts, `!`, and wrapping shifts. For checked arithmetic, where an
    /// out-of-range value must be an overflow error instead, use [`IntValue::checked`].
    pub fn canonical(signed: bool, bits: u8, raw: BigInt) -> Self {
        IntValue {
            signed,
            bits,
            value: wrap(signed, bits, raw),
        }
    }

    /// Inclusive `[min, max]` range for the type.
    pub fn range(signed: bool, bits: u8) -> (BigInt, BigInt) {
        if signed {
            let half = pow2(bits - 1);
            (-half.clone(), half - BigInt::one())
        } else {
            (BigInt::zero(), pow2(bits) - BigInt::one())
        }
    }

    /// Construct from a result of checked arithmetic; error if it does not fit the type.
    pub fn checked(signed: bool, bits: u8, raw: BigInt, op: &str) -> Result<Self, InterpretError> {
        let (min, max) = Self::range(signed, bits);
        if raw < min || raw > max {
            return Err(InterpretError::Overflow(op.to_string()));
        }
        Ok(IntValue {
            signed,
            bits,
            value: raw,
        })
    }

    /// The value as a non-negative integer in `[0, 2^bits)` (two's-complement bit pattern),
    /// used for bitwise ops and conversion to a field element.
    pub fn unsigned_repr(&self) -> BigInt {
        if self.value.sign() == Sign::Minus {
            &self.value + pow2(self.bits)
        } else {
            self.value.clone()
        }
    }

    /// Encode as a field element (the value's bit pattern reduced into the field).
    pub fn to_field(&self) -> FieldElement {
        let (_, bytes) = self.unsigned_repr().to_bytes_be();
        FieldElement::from_be_bytes_reduce(&bytes)
    }
}

impl Value {
    /// Build a tuple, wrapping each field in its own shared cell.
    pub fn tuple(values: Vec<Value>) -> Value {
        Value::Tuple(
            values
                .into_iter()
                .map(|v| Rc::new(RefCell::new(v)))
                .collect(),
        )
    }

    /// Read tuple field `i` as an owned, unaliased value; auto-derefs a reference to a tuple.
    pub fn tuple_field(&self, i: usize) -> Result<Value, InterpretError> {
        match self {
            Value::Tuple(cells) => cells
                .get(i)
                .map(|c| c.borrow().clone().deep_copy())
                .ok_or_else(|| InterpretError::Type(format!("tuple field {i} out of bounds"))),
            Value::Ref(cell, _) => cell.borrow().tuple_field(i),
            other => Err(InterpretError::Type(format!(
                "cannot extract field from {other:?}"
            ))),
        }
    }

    /// Follow a reference one level. A non-`Ref` is a reference shape we don't model (nested or
    /// multi-level) — tolerated as `Unsupported`, not a miscompile.
    pub fn deref(&self) -> Result<Value, InterpretError> {
        match self {
            Value::Ref(cell, _) => Ok(cell.borrow().clone().deep_copy()),
            other => Err(InterpretError::Unsupported(format!(
                "dereference of a non-reference value ({other:?})"
            ))),
        }
    }

    /// Replace every shared cell (in tuples, and through arrays) with a fresh one, so a value read
    /// never aliases a binding. `Ref` keeps its cell — references are meant to share.
    pub fn deep_copy(self) -> Value {
        match self {
            Value::Tuple(cells) => Value::Tuple(
                cells
                    .into_iter()
                    .map(|c| Rc::new(RefCell::new(c.borrow().clone().deep_copy())))
                    .collect(),
            ),
            Value::Array(elements) => {
                Value::Array(elements.into_iter().map(Value::deep_copy).collect())
            }
            other => other,
        }
    }

    pub fn as_bool(&self) -> Result<bool, InterpretError> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(InterpretError::Type(format!(
                "expected bool, got {other:?}"
            ))),
        }
    }

    pub fn as_int(&self) -> Result<&IntValue, InterpretError> {
        match self {
            Value::Int(i) => Ok(i),
            other => Err(InterpretError::Type(format!(
                "expected integer, got {other:?}"
            ))),
        }
    }

    /// Coerce an integer value to a `usize` index.
    pub fn as_index(&self) -> Result<usize, InterpretError> {
        let int = self.as_int()?;
        let repr = int.unsigned_repr();
        let (sign, digits) = repr.to_u64_digits();
        if sign == Sign::Minus || digits.len() > 1 {
            return Err(InterpretError::ValueOutOfRange(format!(
                "index out of usize range: {repr}"
            )));
        }
        Ok(digits.first().copied().unwrap_or(0) as usize)
    }
}
