//! Run records and cross-field comparisons used by the committed ledgers.
//!
//! Across fields, integers, bools and structure must match; `Field` values may differ and failures
//! compare by kind. Across revisions, ledger rows retain exact values and failure payloads.

use std::fmt;

use num_bigint::BigInt;
use serde::{Deserialize, Serialize};

use super::error::InterpretError;
use super::value::{Value, field_to_bigint};

/// Bump whenever the dump shape changes (`RunRecord`, `DiffValue`, `DumpProvenance`), so a stale
/// dump is rejected rather than silently misread.
pub const DUMP_FORMAT_VERSION: u32 = 3;

/// A field-independent encoding of an interpreter [`Value`].
///
/// `Field` carries the element's canonical value in `[0, p)` as a decimal string so a ledger row
/// can print it; the field-axis comparison still treats two field elements as equivalent whatever
/// their values, because they are field-specific by nature. Integers carry their exact value as a
/// `BigInt` so the comparison is precise regardless of width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiffValue {
    Field(String),
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
            Value::Field(field) => DiffValue::Field(field_to_bigint(field).to_string()),
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
            Value::Tuple(cells) => DiffValue::Tuple(
                cells
                    .iter()
                    .map(|c| DiffValue::from_value(&c.borrow()))
                    .collect(),
            ),
            Value::Function(_) => DiffValue::Function,
            // A returned `main` value is Ref-free, but deref defensively so this stays total.
            Value::Ref(cell, _) => DiffValue::from_value(&cell.borrow()),
        }
    }

    /// A compact single-line rendering for ledger rows: `7u64`, `[1u8, 2u8]`, `(true, 3)`.
    pub fn render(&self) -> String {
        match self {
            DiffValue::Field(v) => v.clone(),
            DiffValue::Int {
                signed,
                bits,
                value,
            } => format!("{value}{}{bits}", if *signed { 'i' } else { 'u' }),
            DiffValue::Bool(b) => b.to_string(),
            DiffValue::Unit => "()".to_string(),
            DiffValue::Str(s) => format!("{s:?}"),
            DiffValue::Array(xs) => format!("[{}]", render_list(xs)),
            DiffValue::Tuple(xs) => format!("({})", render_list(xs)),
            DiffValue::Function => "fn".to_string(),
        }
    }
}

fn render_list(values: &[DiffValue]) -> String {
    values
        .iter()
        .map(DiffValue::render)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A normalized failure cause. The field axis compares failures by this kind alone.
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
    /// The interpreter's return disagrees with the `return` recorded in `Prover.toml`. Only ever a
    /// step outcome: the program's verdict stays the value it returned.
    OracleMismatch,
}

impl FailureKind {
    /// The kind as a short label: the variant name, with the construct for `Unsupported`.
    pub fn label(&self) -> String {
        match self {
            FailureKind::Unsupported { construct } => format!("Unsupported({construct})"),
            other => format!("{other:?}"),
        }
    }
}

/// A failure reduced to what two runs of one program can be compared on: the kind, and the
/// payload that tells two failures of the same kind apart (the normalized assertion message, the
/// overflowing operation, the unsupported construct, the compiler's diagnostic text). Triage
/// detail that is neither compared nor written to the ledger lives beside it, never inside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparableError {
    pub kind: FailureKind,
    pub payload: String,
}

impl ComparableError {
    pub fn new(kind: FailureKind, payload: impl Into<String>) -> Self {
        ComparableError {
            kind,
            payload: payload.into(),
        }
    }
}

impl fmt::Display for ComparableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.payload.is_empty() {
            write!(f, "{}", self.kind.label())
        } else {
            write!(f, "{}: {}", self.kind.label(), self.payload)
        }
    }
}

/// The outcome of interpreting one program under one field, ready to serialize and diff. `detail`
/// on `Errored` is human-readable triage text only: it is never compared and never in a ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiffOutcome {
    Returned(DiffValue),
    Errored {
        error: ComparableError,
        detail: String,
    },
}

/// Project an [`InterpretError`] onto its comparable kind and payload.
pub fn comparable_error_of(error: &InterpretError) -> ComparableError {
    let (kind, payload) = match error {
        InterpretError::AssertionFailed { message, .. } => (
            FailureKind::AssertionFailed,
            message.as_deref().unwrap_or_default(),
        ),
        InterpretError::Overflow(op) => (FailureKind::Overflow, op.as_str()),
        InterpretError::DivisionByZero => (FailureKind::DivisionByZero, ""),
        InterpretError::ValueOutOfRange(m) => (FailureKind::ValueOutOfRange, m.as_str()),
        InterpretError::InvalidInput(m) => (FailureKind::InputError, m.as_str()),
        InterpretError::Type(m) => (FailureKind::TypeError, m.as_str()),
        InterpretError::Unsupported(msg) => (
            FailureKind::Unsupported {
                construct: normalize_construct(msg),
            },
            msg.as_str(),
        ),
        InterpretError::Internal(m) => (FailureKind::Internal, m.as_str()),
    };
    ComparableError::new(kind, normalize_text(payload))
}

/// The comparable kind of an [`InterpretError`], without its payload.
pub fn failure_kind_of(error: &InterpretError) -> FailureKind {
    comparable_error_of(error).kind
}

/// The construct kind is the head of an `Unsupported` message, before any name, value or detail.
pub(crate) fn normalize_construct(msg: &str) -> String {
    msg.split([':', '\'', '('])
        .next()
        .unwrap_or(msg)
        .trim()
        .to_string()
}

/// Collapse every whitespace run to one space and trim, so a payload is one stable line.
pub fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One step of a program's run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepOutcome {
    Passed,
    Failed {
        error: ComparableError,
        detail: String,
    },
    /// The step did not run, typically because an earlier one failed.
    NotRun {
        reason: String,
    },
}

impl StepOutcome {
    pub fn failed(error: ComparableError, detail: impl Into<String>) -> Self {
        StepOutcome::Failed {
            error,
            detail: detail.into(),
        }
    }

    pub fn not_run(reason: impl Into<String>) -> Self {
        StepOutcome::NotRun {
            reason: reason.into(),
        }
    }

    pub fn passed(&self) -> bool {
        matches!(self, StepOutcome::Passed)
    }

    pub fn failure(&self) -> Option<&ComparableError> {
        match self {
            StepOutcome::Failed { error, .. } => Some(error),
            _ => None,
        }
    }

    /// The ledger cell for this step: `ok`, `FAIL <kind>: <payload>` or `n/a: <reason>`.
    pub fn render(&self) -> String {
        match self {
            StepOutcome::Passed => "ok".to_string(),
            StepOutcome::Failed { error, .. } => format!("FAIL {error}"),
            StepOutcome::NotRun { reason } => format!("n/a: {reason}"),
        }
    }
}

/// Everything one run of one program produced, step by step. Each step runs under its own panic
/// guard, so a panic is recorded where it happened and the steps after it read `NotRun`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Hash of the program's normalized sources (`Nargo.toml`, `Prover.toml`, `src/**`).
    pub source_hash: String,
    pub load: StepOutcome,
    pub compile: StepOutcome,
    pub interpret: StepOutcome,
    /// The interpreter's return checked against the `return` recorded in `Prover.toml`.
    pub oracle: StepOutcome,
    /// Canonicalizing and hashing the monomorphized program.
    pub projection: StepOutcome,
    pub returned: Option<DiffValue>,
    pub projection_hash: Option<String>,
}

impl RunRecord {
    /// The per-program verdict the field-axis comparison consumes: the returned value, else the
    /// first failed step. The oracle and projection steps never change it.
    pub fn outcome(&self) -> DiffOutcome {
        if let Some(value) = &self.returned {
            return DiffOutcome::Returned(value.clone());
        }
        for step in [&self.load, &self.compile, &self.interpret] {
            if let StepOutcome::Failed { error, detail } = step {
                return DiffOutcome::Errored {
                    error: error.clone(),
                    detail: detail.clone(),
                };
            }
        }
        DiffOutcome::Errored {
            error: ComparableError::new(
                FailureKind::Internal,
                "record has neither a returned value nor a failed step",
            ),
            detail: String::new(),
        }
    }
}

fn is_kind(outcome: &DiffOutcome, kind: &FailureKind) -> bool {
    matches!(outcome, DiffOutcome::Errored { error, .. } if &error.kind == kind)
}

fn is_panic(outcome: &DiffOutcome) -> bool {
    is_kind(outcome, &FailureKind::Panic)
}

fn is_internal(outcome: &DiffOutcome) -> bool {
    is_kind(outcome, &FailureKind::Internal)
}

/// Whether this outcome provides no executable result to compare.
pub fn is_coverage_gap(outcome: &DiffOutcome) -> bool {
    matches!(
        outcome,
        DiffOutcome::Errored { error, .. }
            if matches!(
                error.kind,
                FailureKind::Unsupported { .. } | FailureKind::DependencyCompileGap
            )
    )
}

fn outcome_summary(outcome: &DiffOutcome) -> String {
    match outcome {
        DiffOutcome::Returned(value) => format!("returned {}", value.render()),
        DiffOutcome::Errored { error, .. } => format!("errored ({error})"),
    }
}

/// Compare cross-field outcomes, tolerating countable coverage gaps while rejecting value, error,
/// panic, and internal outcomes. Failures compare by kind: payloads are field-specific text.
pub fn outcomes_equivalent(a: &DiffOutcome, b: &DiffOutcome) -> Result<(), String> {
    if is_panic(a) || is_panic(b) {
        return Err(format!(
            "panic outcome: {} vs {}",
            outcome_summary(a),
            outcome_summary(b)
        ));
    }
    if is_internal(a) || is_internal(b) {
        return Err(format!(
            "internal error outcome: {} vs {}",
            outcome_summary(a),
            outcome_summary(b)
        ));
    }

    if is_coverage_gap(a) || is_coverage_gap(b) {
        return Ok(());
    }
    match (a, b) {
        (DiffOutcome::Returned(x), DiffOutcome::Returned(y)) => values_equivalent(x, y),
        (DiffOutcome::Returned(_), DiffOutcome::Errored { error, .. }) => Err(format!(
            "one field returned a value, the other errored ({error})"
        )),
        (DiffOutcome::Errored { error, .. }, DiffOutcome::Returned(_)) => Err(format!(
            "one field errored ({error}), the other returned a value"
        )),
        (DiffOutcome::Errored { error: ea, .. }, DiffOutcome::Errored { error: eb, .. }) => {
            if ea.kind == eb.kind {
                Ok(())
            } else {
                Err(format!(
                    "both errored, but differently: {} vs {}",
                    ea.kind.label(),
                    eb.kind.label()
                ))
            }
        }
    }
}

/// Whether equivalence depends on tolerating a coverage gap.
pub fn outcome_is_tolerated(a: &DiffOutcome, b: &DiffOutcome) -> bool {
    !is_panic(a)
        && !is_panic(b)
        && !is_internal(a)
        && !is_internal(b)
        && (is_coverage_gap(a) || is_coverage_gap(b))
}

/// A dump's provenance, stamped when it is written so two dumps from mismatched builds are rejected
/// rather than diffed into phantom divergences. The ledger header prints the reproducible subset;
/// `interpreter_rev`, `interpreter_dirty`, `corpus_dir` and `built_at` are triage fields that only
/// the JSON dump carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DumpProvenance {
    pub format_version: u32,
    pub projection_version: u32,
    pub field: String,
    pub field_modulus: String,
    /// The compiler's own build-script stamp (`noirc_driver::GIT_COMMIT`): the commit the pinned
    /// crates were built from, whatever `Cargo.toml` claims.
    pub noir_rev: String,
    pub interpreter_rev: String,
    pub interpreter_dirty: bool,
    pub corpus_dir: String,
    /// Hash over every program's `source_hash`, in name order.
    pub corpus_hash: String,
    pub program_count: usize,
    /// `rustc --version` of the toolchain that built the referee and the compiler.
    pub toolchain: String,
    /// The referee's enabled cargo features, sorted.
    pub features: Vec<String>,
    pub built_at: String,
}

/// One field's corpus records plus the provenance needed to trust a cross-field diff of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossFieldDump {
    pub provenance: DumpProvenance,
    /// Records in program-name order.
    pub records: Vec<(String, RunRecord)>,
}

/// Parse a dump, refusing any format version but the current one by name.
#[cfg(test)]
pub(crate) fn parse_dump(json: &str) -> Result<CrossFieldDump, String> {
    #[derive(Deserialize)]
    struct Probe {
        provenance: ProbeProvenance,
    }
    #[derive(Deserialize)]
    struct ProbeProvenance {
        format_version: u32,
    }
    let probe: Probe =
        serde_json::from_str(json).map_err(|e| format!("dump has no readable provenance: {e}"))?;
    if probe.provenance.format_version != DUMP_FORMAT_VERSION {
        return Err(format!(
            "dump is format {}; this referee expects {DUMP_FORMAT_VERSION} (regenerate it under both fields)",
            probe.provenance.format_version
        ));
    }
    serde_json::from_str(json).map_err(|e| format!("dump did not parse: {e}"))
}

/// Whether two values are cross-field equivalent: integers/bools/structure must match exactly,
/// `Field` values may differ.
pub fn values_equivalent(a: &DiffValue, b: &DiffValue) -> Result<(), String> {
    match (a, b) {
        // Field values are field-specific; any difference there is expected.
        (DiffValue::Field(_), DiffValue::Field(_)) => Ok(()),
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

    fn field(value: &str) -> DiffValue {
        DiffValue::Field(value.to_string())
    }

    fn int(value: &str) -> DiffValue {
        DiffValue::Int {
            signed: false,
            bits: 64,
            value: value.parse().unwrap(),
        }
    }

    #[test]
    fn integer_values_must_match() {
        assert!(values_equivalent(&int("42"), &int("42")).is_ok());
        assert!(values_equivalent(&int("42"), &int("43")).is_err());
    }

    #[test]
    fn nested_integer_difference_is_found() {
        let a = DiffValue::Tuple(vec![field("5"), DiffValue::Array(vec![int("1"), int("2")])]);
        let b = DiffValue::Tuple(vec![field("6"), DiffValue::Array(vec![int("1"), int("9")])]);
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
            error: ComparableError::new(kind, ""),
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
    fn unsupported_is_tolerated_and_counted() {
        let u = unsupported();
        let c = errored(FailureKind::CompileError);
        assert!(outcomes_equivalent(&u, &c).is_ok());
        assert!(outcome_is_tolerated(&u, &c));
    }

    #[test]
    fn two_panics_diverge() {
        let p = errored(FailureKind::Panic);
        assert!(outcomes_equivalent(&p, &p).is_err());
        assert!(!outcome_is_tolerated(&p, &p));
    }

    #[test]
    fn internal_errors_never_agree() {
        let i = errored(FailureKind::Internal);
        assert!(outcomes_equivalent(&i, &i).is_err());
        assert!(outcomes_equivalent(&i, &unsupported()).is_err());
        assert!(!outcome_is_tolerated(&i, &unsupported()));
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

    fn cmp(kind: FailureKind, payload: &str) -> ComparableError {
        ComparableError {
            kind,
            payload: payload.to_string(),
        }
    }

    fn failed(kind: FailureKind, payload: &str) -> DiffOutcome {
        DiffOutcome::Errored {
            error: cmp(kind, payload),
            detail: String::new(),
        }
    }

    #[test]
    fn field_values_carry_their_value_and_stay_field_equivalent() {
        let a = DiffValue::Field("1".to_string());
        let b = DiffValue::Field("18446744069414584320".to_string());
        assert!(values_equivalent(&a, &b).is_ok());
        assert_ne!(a, b);
    }

    #[test]
    fn same_kind_different_payload_is_field_equivalent_but_distinct() {
        let a = failed(FailureKind::AssertionFailed, "x == 1");
        let b = failed(FailureKind::AssertionFailed, "x == 2");
        assert!(outcomes_equivalent(&a, &b).is_ok());
        assert_ne!(a, b);
    }

    #[test]
    fn overflow_payload_is_the_operation() {
        let error = comparable_error_of(&InterpretError::Overflow("addition on u64".to_string()));
        assert_eq!(error.kind, FailureKind::Overflow);
        assert_eq!(error.payload, "addition on u64");
    }

    #[test]
    fn assertion_payload_is_the_whitespace_normalized_message() {
        let error = comparable_error_of(&InterpretError::AssertionFailed {
            location: noirc_errors::Location::dummy(),
            message: Some("  a   ==\n b ".to_string()),
        });
        assert_eq!(error.kind, FailureKind::AssertionFailed);
        assert_eq!(error.payload, "a == b");
    }

    #[test]
    fn unsupported_payload_keeps_the_whole_message_and_the_kind_keeps_the_construct() {
        let error = comparable_error_of(&InterpretError::Unsupported(
            "intrinsic 'foo': bar".to_string(),
        ));
        assert_eq!(
            error.kind,
            FailureKind::Unsupported {
                construct: "intrinsic".to_string()
            }
        );
        assert_eq!(error.payload, "intrinsic 'foo': bar");

        let error = comparable_error_of(&InterpretError::Unsupported(
            "dereference of a non-reference value (Tuple([RefCell { value: Field(0) }]))"
                .to_string(),
        ));
        assert_eq!(
            error.kind,
            FailureKind::Unsupported {
                construct: "dereference of a non-reference value".to_string()
            }
        );
    }

    fn record() -> RunRecord {
        RunRecord {
            source_hash: "00".repeat(32),
            load: StepOutcome::Passed,
            compile: StepOutcome::Passed,
            interpret: StepOutcome::Passed,
            oracle: StepOutcome::not_run("no recorded return"),
            projection: StepOutcome::Passed,
            returned: Some(int("7")),
            projection_hash: Some("11".repeat(32)),
        }
    }

    #[test]
    fn record_with_a_value_returns_it() {
        assert_eq!(record().outcome(), DiffOutcome::Returned(int("7")));
    }

    #[test]
    fn record_outcome_is_the_first_failed_step() {
        let mut r = record();
        r.compile = StepOutcome::failed(cmp(FailureKind::CompileError, "no main"), "detail");
        r.interpret = StepOutcome::not_run("not compiled");
        r.returned = None;
        r.projection = StepOutcome::not_run("not compiled");
        r.projection_hash = None;
        assert_eq!(
            r.outcome(),
            failed_with_detail(FailureKind::CompileError, "no main", "detail")
        );
    }

    fn failed_with_detail(kind: FailureKind, payload: &str, detail: &str) -> DiffOutcome {
        DiffOutcome::Errored {
            error: cmp(kind, payload),
            detail: detail.to_string(),
        }
    }

    #[test]
    fn an_interpret_panic_is_recorded_and_never_equivalent() {
        let mut r = record();
        r.interpret = StepOutcome::failed(cmp(FailureKind::Panic, "index out of bounds"), "");
        r.returned = None;
        let outcome = r.outcome();
        assert!(matches!(
            &outcome,
            DiffOutcome::Errored { error, .. } if error.kind == FailureKind::Panic
        ));
        assert!(outcomes_equivalent(&outcome, &record().outcome()).is_err());
        assert!(outcomes_equivalent(&outcome, &outcome).is_err());
    }

    #[test]
    fn an_oracle_mismatch_does_not_change_the_verdict() {
        // The recorded-return check is a row of its own; the field-axis verdict stays the value.
        let mut r = record();
        r.oracle = StepOutcome::failed(cmp(FailureKind::OracleMismatch, "integer differs"), "");
        assert_eq!(r.outcome(), DiffOutcome::Returned(int("7")));
    }

    #[test]
    fn a_stale_format_dump_is_rejected_with_the_versions_named() {
        let stale = r#"{"provenance":{"format_version":2,"field":"bn254","field_modulus":"1","noir_rev":"x","interpreter_rev":"y","corpus_dir":"z","program_count":0,"built_at":""},"outcomes":[]}"#;
        let err = parse_dump(stale).unwrap_err();
        assert!(err.contains("format 2"), "{err}");
        assert!(err.contains("expects 3"), "{err}");
    }
}
