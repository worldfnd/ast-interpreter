//! Cross-field differential: compare the interpreter's result for the same program compiled under
//! two fields (bn254 vs Goldilocks). Each build dumps its outcomes to JSON; a separate step diffs
//! them. Integers, bools, and structure must match; `Field` values may differ. The comparison is
//! field-agnostic, so it can be unit-tested without a full Goldilocks corpus run.

use num_bigint::BigInt;
use serde::{Deserialize, Serialize};

use super::error::InterpretError;
use super::value::Value;

/// Bump whenever `DiffOutcome` or `DumpProvenance` changes shape, so a stale dump is rejected
/// rather than silently misread.
pub const DUMP_FORMAT_VERSION: u32 = 2;

/// A field-independent encoding of an interpreter [`Value`] for cross-field comparison.
///
/// `Field` is intentionally opaque (carries no value): field elements are field-specific, so a
/// difference there is expected, not a divergence. Integers carry their exact value as a `BigInt`
/// so the comparison is precise regardless of width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiffValue {
    Field,
    Int {
        signed: bool,
        bits: u8,
        value: BigInt,
    },
    Bool(bool),
    Unit,
    Str(String),
    Array(Vec<DiffValue>),
    Tuple(Vec<DiffValue>),
    Function,
}

impl DiffValue {
    pub fn from_value(value: &Value) -> DiffValue {
        match value {
            Value::Field(_) => DiffValue::Field,
            Value::Int(int) => DiffValue::Int {
                signed: int.signed,
                bits: int.bits,
                value: int.value.clone(),
            },
            Value::Bool(b) => DiffValue::Bool(*b),
            Value::Unit => DiffValue::Unit,
            Value::Str(s) => DiffValue::Str(s.clone()),
            Value::Array(elements) => {
                DiffValue::Array(elements.iter().map(DiffValue::from_value).collect())
            }
            Value::Tuple(elements) => {
                DiffValue::Tuple(elements.iter().map(DiffValue::from_value).collect())
            }
            Value::Function(_) => DiffValue::Function,
        }
    }
}

/// A normalized failure cause for comparing cross-field outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FailureKind {
    ProjectLoad,
    CompileError,
    DependencyCompileGap,
    InputError,
    /// Normalized unsupported construct kind.
    Unsupported {
        construct: String,
    },
    AssertionFailed,
    Overflow,
    DivisionByZero,
    ValueOutOfRange,
    TypeError,
    Internal,
    /// The compiler or interpreter panicked; never equivalent to a non-panic outcome.
    Panic,
}

/// The outcome of interpreting one program under one field, ready to serialize and diff. `detail`
/// on `Errored` is human-readable triage text only — it is never compared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiffOutcome {
    Returned(DiffValue),
    Errored { kind: FailureKind, detail: String },
}

/// Project an [`InterpretError`] onto its comparable [`FailureKind`]. The `String` payloads carry
/// only triage detail, so they are dropped here; `Unsupported` keeps a normalized construct kind.
pub fn failure_kind_of(error: &InterpretError) -> FailureKind {
    match error {
        InterpretError::AssertionFailed { .. } => FailureKind::AssertionFailed,
        InterpretError::Overflow(_) => FailureKind::Overflow,
        InterpretError::DivisionByZero => FailureKind::DivisionByZero,
        InterpretError::ValueOutOfRange(_) => FailureKind::ValueOutOfRange,
        InterpretError::InvalidInput(_) => FailureKind::InputError,
        InterpretError::Type(_) => FailureKind::TypeError,
        InterpretError::Unsupported(msg) => FailureKind::Unsupported {
            construct: normalize_construct(msg),
        },
        InterpretError::Internal(_) => FailureKind::Internal,
    }
}

/// The construct kind is the head of an `Unsupported` message, before any name or detail.
pub(crate) fn normalize_construct(msg: &str) -> String {
    msg.split([':', '\''])
        .next()
        .unwrap_or(msg)
        .trim()
        .to_string()
}

fn is_panic(outcome: &DiffOutcome) -> bool {
    matches!(
        outcome,
        DiffOutcome::Errored {
            kind: FailureKind::Panic,
            ..
        }
    )
}

/// Whether this outcome provides no executable result to compare.
fn is_coverage_gap(outcome: &DiffOutcome) -> bool {
    matches!(
        outcome,
        DiffOutcome::Errored {
            kind: FailureKind::Unsupported { .. } | FailureKind::DependencyCompileGap,
            ..
        }
    )
}

/// Compare cross-field outcomes, tolerating countable coverage gaps while rejecting value, error,
/// and one-sided panic mismatches.
pub fn outcomes_equivalent(a: &DiffOutcome, b: &DiffOutcome) -> Result<(), String> {
    if is_panic(a) || is_panic(b) {
        return match (a, b) {
            (DiffOutcome::Errored { kind: ka, .. }, DiffOutcome::Errored { kind: kb, .. })
                if ka == kb =>
            {
                Ok(())
            }
            _ => Err(format!("crash on one side: {a:?} vs {b:?}")),
        };
    }

    if is_coverage_gap(a) || is_coverage_gap(b) {
        return Ok(());
    }
    match (a, b) {
        (DiffOutcome::Returned(x), DiffOutcome::Returned(y)) => values_equivalent(x, y),
        (DiffOutcome::Returned(_), DiffOutcome::Errored { kind, detail }) => Err(format!(
            "one field returned a value, the other errored ({kind:?}: {detail})"
        )),
        (DiffOutcome::Errored { kind, detail }, DiffOutcome::Returned(_)) => Err(format!(
            "one field errored ({kind:?}: {detail}), the other returned a value"
        )),
        (DiffOutcome::Errored { kind: ka, .. }, DiffOutcome::Errored { kind: kb, .. }) => {
            if ka == kb {
                Ok(())
            } else {
                Err(format!("both errored, but differently: {ka:?} vs {kb:?}"))
            }
        }
    }
}

/// Whether equivalence depends on tolerating a coverage gap.
pub fn outcome_is_tolerated(a: &DiffOutcome, b: &DiffOutcome) -> bool {
    !is_panic(a) && !is_panic(b) && (is_coverage_gap(a) || is_coverage_gap(b))
}

/// A dump's provenance, stamped when it is written so two dumps from mismatched builds are rejected
/// rather than diffed into phantom divergences. `built_at` is triage only and never compared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DumpProvenance {
    pub format_version: u32,
    pub field: String,
    pub field_modulus: String,
    pub noir_rev: String,
    pub interpreter_rev: String,
    pub corpus_dir: String,
    pub program_count: usize,
    pub built_at: String,
}

/// One field's corpus outcomes plus the provenance needed to trust a cross-field diff of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossFieldDump {
    pub provenance: DumpProvenance,
    pub outcomes: Vec<(String, DiffOutcome)>,
}

/// Whether two values are cross-field equivalent: integers/bools/structure must match exactly,
/// `Field` values may differ.
pub fn values_equivalent(a: &DiffValue, b: &DiffValue) -> Result<(), String> {
    match (a, b) {
        // Field values are field-specific; any difference there is expected.
        (DiffValue::Field, DiffValue::Field) => Ok(()),
        (DiffValue::Function, DiffValue::Function) => Ok(()),
        (DiffValue::Unit, DiffValue::Unit) => Ok(()),
        (DiffValue::Bool(x), DiffValue::Bool(y)) => {
            if x == y {
                Ok(())
            } else {
                Err(format!("bool differs: {x} vs {y}"))
            }
        }
        (DiffValue::Str(x), DiffValue::Str(y)) => {
            if x == y {
                Ok(())
            } else {
                Err(format!("string differs: {x:?} vs {y:?}"))
            }
        }
        (
            DiffValue::Int {
                signed: s1,
                bits: b1,
                value: v1,
            },
            DiffValue::Int {
                signed: s2,
                bits: b2,
                value: v2,
            },
        ) => {
            if s1 == s2 && b1 == b2 && v1 == v2 {
                Ok(())
            } else {
                Err(format!(
                    "integer differs: {v1} (s={s1},{b1}b) vs {v2} (s={s2},{b2}b)"
                ))
            }
        }
        (DiffValue::Array(xs), DiffValue::Array(ys)) => elementwise(xs, ys, "array"),
        (DiffValue::Tuple(xs), DiffValue::Tuple(ys)) => elementwise(xs, ys, "tuple"),
        (x, y) => Err(format!("shape differs: {x:?} vs {y:?}")),
    }
}

fn elementwise(xs: &[DiffValue], ys: &[DiffValue], kind: &str) -> Result<(), String> {
    if xs.len() != ys.len() {
        return Err(format!(
            "{kind} length differs: {} vs {}",
            xs.len(),
            ys.len()
        ));
    }
    for (i, (x, y)) in xs.iter().zip(ys).enumerate() {
        values_equivalent(x, y).map_err(|e| format!("{kind}[{i}]: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(value: &str) -> DiffValue {
        DiffValue::Int {
            signed: false,
            bits: 64,
            value: value.parse().unwrap(),
        }
    }

    #[test]
    fn equal_integers_are_equivalent() {
        assert!(values_equivalent(&int("42"), &int("42")).is_ok());
    }

    #[test]
    fn differing_integers_diverge() {
        // This is the corruption signal: the same program produced different integers per field.
        assert!(values_equivalent(&int("42"), &int("43")).is_err());
    }

    #[test]
    fn field_values_may_differ() {
        // Field elements are field-specific; a difference there is not a divergence.
        assert!(values_equivalent(&DiffValue::Field, &DiffValue::Field).is_ok());
    }

    #[test]
    fn nested_integer_difference_is_found() {
        let a = DiffValue::Tuple(vec![
            DiffValue::Field,
            DiffValue::Array(vec![int("1"), int("2")]),
        ]);
        let b = DiffValue::Tuple(vec![
            DiffValue::Field,
            DiffValue::Array(vec![int("1"), int("9")]),
        ]);
        let err = values_equivalent(&a, &b).unwrap_err();
        assert!(
            err.contains("tuple[1]"),
            "path should point at the mismatch: {err}"
        );
    }

    #[test]
    fn shape_mismatch_diverges() {
        assert!(values_equivalent(&int("0"), &DiffValue::Bool(false)).is_err());
    }

    fn errored(kind: FailureKind) -> DiffOutcome {
        DiffOutcome::Errored {
            kind,
            detail: String::new(),
        }
    }

    fn unsupported() -> DiffOutcome {
        errored(FailureKind::Unsupported {
            construct: "intrinsic".to_string(),
        })
    }

    #[test]
    fn returned_vs_real_error_diverges() {
        let a = DiffOutcome::Returned(int("1"));
        let b = errored(FailureKind::CompileError);
        assert!(outcomes_equivalent(&a, &b).is_err());
        assert!(!outcome_is_tolerated(&a, &b));
    }

    #[test]
    fn returned_vs_coverage_gap_is_tolerated_and_counted() {
        let ran = DiffOutcome::Returned(int("1"));
        for gap in [unsupported(), errored(FailureKind::DependencyCompileGap)] {
            assert!(outcomes_equivalent(&ran, &gap).is_ok());
            assert!(outcome_is_tolerated(&ran, &gap));
            assert!(outcomes_equivalent(&gap, &ran).is_ok());
            assert!(outcome_is_tolerated(&gap, &ran));
        }
    }

    #[test]
    fn same_kind_errors_are_equivalent() {
        let a = errored(FailureKind::CompileError);
        assert!(outcomes_equivalent(&a, &a).is_ok());
    }

    #[test]
    fn unsupported_is_tolerated_and_counted() {
        let u = unsupported();
        let c = errored(FailureKind::CompileError);
        assert!(outcomes_equivalent(&u, &c).is_ok());
        assert!(outcome_is_tolerated(&u, &c));
    }

    #[test]
    fn two_panics_are_treated_as_agreement() {
        let p = errored(FailureKind::Panic);
        assert!(outcomes_equivalent(&p, &p).is_ok());
        assert!(!outcome_is_tolerated(&p, &p));
    }

    #[test]
    fn panic_is_never_tolerated() {
        let p = errored(FailureKind::Panic);
        for gap in [unsupported(), errored(FailureKind::DependencyCompileGap)] {
            assert!(outcomes_equivalent(&p, &gap).is_err());
            assert!(!outcome_is_tolerated(&p, &gap));
            assert!(outcomes_equivalent(&gap, &p).is_err());
            assert!(!outcome_is_tolerated(&gap, &p));
        }
    }

    #[test]
    fn differing_real_kinds_diverge() {
        let a = errored(FailureKind::Overflow);
        let b = errored(FailureKind::AssertionFailed);
        assert!(outcomes_equivalent(&a, &b).is_err());
    }
}
