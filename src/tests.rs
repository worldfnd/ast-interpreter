use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use super::diff::{
    CrossFieldDump, DUMP_FORMAT_VERSION, DumpProvenance, FailureKind, failure_kind_of,
    outcome_is_tolerated,
};
use super::loader::NoirProject;
use super::validation_frontend::compile_for_validation;
use super::{
    DiffOutcome, DiffValue, IntValue, InterpretError, Value, inputs_from_prover_toml,
    interpret_with_inputs, outcomes_equivalent,
};
#[cfg(not(feature = "goldilocks"))]
use super::{expected_return_from_prover_toml, interpret};
use acvm::{AcirField, FieldElement};
use num_bigint::BigInt;

fn validation_failure_kind(error: &super::validation_frontend::ValidationError) -> FailureKind {
    if error.is_dependency_compile_gap() {
        FailureKind::DependencyCompileGap
    } else {
        FailureKind::CompileError
    }
}

/// A test Noir package under `fixtures/`. Positive packages keep a plain name; negatives carry a
/// `neg_` prefix (built via [`negative_fixture`]).
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// A `neg_`-prefixed fixture: a program expected to fail to compile or assert.
#[cfg(not(feature = "goldilocks"))]
fn negative_fixture(name: &str) -> PathBuf {
    fixture(&format!("neg_{name}"))
}

/// Compile a fixture through Noir's frontend + monomorphizer and interpret the resulting AST.
#[cfg(not(feature = "goldilocks"))]
fn interpret_fixture(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let project = NoirProject::new(fixture(name))?;
    let validated = compile_for_validation(&project)?;
    Ok(interpret(&validated.program)?)
}

/// Under bn254 the self-checking `interp_basic` program interprets to a clean `Unit` with every
/// `assert` holding — the interpreter agrees with Noir's semantics on real monomorphized output.
/// Gated off under goldilocks because the auto-injected bn254 stdlib does not compile for that field.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_basic_corpus_program() {
    let result = interpret_fixture("interp_basic").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}

/// The oracle must bite: a program whose computed (non-const-folded) assertion is false interprets
/// to `AssertionFailed`, not a clean pass — without this the passing case above would be hollow.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn detects_false_assertion() {
    let project = NoirProject::new(negative_fixture("assert_fail")).expect("project");
    let validated = compile_for_validation(&project).expect("frontend compile");
    match interpret(&validated.program) {
        Err(InterpretError::AssertionFailed { .. }) => {}
        other => panic!("expected AssertionFailed, got {other:?}"),
    }
}

/// The validation frontend tolerates elaboration errors only in code `main` does not reach; a type
/// error in the *reachable* set must still be rejected, or the oracle would validate a
/// silently-miscompiled AST. The fixture binds a `u64` to a `bool` in `main`'s own body, so the
/// error is unambiguously reachable. A clean `Ok(())` is the hole we guard against; a monomorphizer
/// panic on the elaborator's `Error` node is also a rejection, so both count as a pass.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn rejects_reachable_type_error() {
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let project = NoirProject::new(negative_fixture("reachable_error")).expect("project");
        compile_for_validation(&project).map(|_| ())
    }));
    match outcome {
        Ok(Err(_)) => {} // rejected cleanly — desired
        Err(_) => {}     // monomorphizer panicked on the Error node — also a rejection
        Ok(Ok(())) => panic!(
            "reachable type error was silently accepted: the validation frontend produced a \
             mono-AST for un-type-checkable reachable code — oracle false-confidence hole"
        ),
    }
}

/// Reached dependency code with tolerated diagnostics must be rejected before interpretation.
#[cfg(feature = "goldilocks")]
#[test]
fn rejects_reached_dependency_error() {
    let project = NoirProject::new(fixture("interp_reached_dep_error")).expect("project");
    let err = match compile_for_validation(&project) {
        Ok(_) => panic!(
            "a program reaching code from a tolerated-error file must be rejected, not validated"
        ),
        Err(e) => e,
    };
    assert!(
        err.is_dependency_compile_gap(),
        "expected the tolerated-file invariant rejection, got: {err}"
    );
}

/// The same dependency fixture compiles and interprets under bn254.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_reached_dep_fixture_on_bn254() {
    let project = NoirProject::new(fixture("interp_reached_dep_error")).expect("project");
    let validated = compile_for_validation(&project).expect("clean compile under bn254");
    let x = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(3u64),
    });
    let result = interpret_with_inputs(&validated.program, vec![x]).expect("interpret");
    let expected = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(2u64),
    });
    assert_eq!(result, expected);
}

/// The `Prover.toml` input bridge, exercised from an always-present fixture (the corpus tests below
/// skip when the Noir checkout is absent). `interp_inputs_u64` sets `x = 3` and computes
/// `x * 2 + (p + 1)` natively in u64 — the same value under either field.
#[test]
fn interprets_fixture_inputs_from_prover_toml() {
    let root = fixture("interp_inputs_u64");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");

    assert!(matches!(
        interpret_with_inputs(&validated.program, Vec::new()),
        Err(InterpretError::InvalidInput(_))
    ));
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    let expected = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(18446744069414584328u64),
    });
    assert_eq!(result, expected, "input bridge must feed x = 3");
}

/// Signed i32 inputs decode identically on both fields (32-bit two's complement fits every
/// modulus) and drive signed negation, truncating division, sign-carrying remainder, and
/// comparison. With `a = -7, b = 2`: `7 + (-3)*10 + (-1)*100 + 2 == -121`.
#[test]
fn interprets_signed_i32_input() {
    let root = fixture("interp_inputs_i32");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: true,
            bits: 32,
            value: BigInt::from(-121)
        })
    );
}

/// bn254 positive control for the i64 guard: with 2^64 < p the two's-complement encoding is
/// injective, so `x = -1` decodes correctly.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn bn254_decodes_signed_i64_input() {
    let root = fixture("neg_interp_inputs_i64");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: true,
            bits: 64,
            value: BigInt::from(-1)
        })
    );
}

/// No i64 input is silently decoded under Goldilocks. A negative magnitude is refused by the ABI
/// parser (it exceeds the field modulus); an in-field positive is refused by the representability
/// guard (it could be a wrapped negative, since i64's 2^64 range does not fit the field).
#[cfg(feature = "goldilocks")]
#[test]
fn goldilocks_rejects_unrepresentable_i64_input() {
    let root = fixture("neg_interp_inputs_i64");
    let project = NoirProject::new(root).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    assert!(
        inputs_from_prover_toml(&validated.program, &validated.abi, "x = \"-1\"").is_err(),
        "goldilocks must reject a negative i64 input"
    );
    match inputs_from_prover_toml(&validated.program, &validated.abi, "x = \"1\"") {
        Err(InterpretError::Unsupported(_)) => {}
        other => panic!(
            "the i64 representability guard should reject an in-field value too, got {other:?}"
        ),
    }
}

/// Struct inputs map by declaration order, not the alphabetical ABI map, at every nesting level.
/// The fixture nests a `Pair` inside an `Outer` and puts both structs' fields out of alphabetical
/// order, and reaches through an array field, so a positional-on-alphabetical bridge would corrupt
/// the result: `zeta*1000 + alpha*100 + (1+2+3) == 3706`.
#[test]
fn interprets_struct_input_by_declaration_order() {
    let root = fixture("interp_inputs_struct");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: false,
            bits: 32,
            value: BigInt::from(3706)
        })
    );
}

/// An array input plus a helper call, a loop with indexing, and a signed conditional, all fed from
/// `Prover.toml`, exercise the input bridge and interpreter together. With `xs = [10,20,30,40]`
/// (weighted `10*1 + 20*2 + 30*3 + 40*4 == 300`) and `k = -3` (negative branch), the result is
/// `300 - 5 == 295`. Every input is field-independent, so this holds under bn254 and Goldilocks.
#[test]
fn interprets_mixed_inputs() {
    let root = fixture("interp_inputs_mixed");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: false,
            bits: 32,
            value: BigInt::from(295)
        })
    );
}

/// A `&mut` parameter mutates a caller local through a call, and a local `&mut` writes through a deref.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_scalar_references() {
    let result = interpret_fixture("interp_refs_scalar").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}

/// `&mut s.field` aliases one field cell (sibling untouched), and a field ref taken before a
/// whole-struct reassignment still observes the new value.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_struct_field_references() {
    let result =
        interpret_fixture("interp_refs_struct_field").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}

/// A `&mut` threaded through `main -> twice -> bump` mutates one shared cell: `100 + 5 + 5 == 110`.
#[test]
fn interprets_reference_call_chain() {
    let root = fixture("interp_refs_call_chain");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result = interpret_with_inputs(&validated.program, inputs).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: false,
            bits: 64,
            value: BigInt::from(110)
        })
    );
}

/// `&mut holder.inner_ref.v` peels through a ref-typed field to the live cell, so the write aliases
/// the original `inner` (sibling untouched) — the multi-layer place peel + the `skip` case.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_reference_through_struct_field() {
    let result =
        interpret_fixture("interp_refs_nested_field").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}

/// Two copies of the same inner `&mut` alias, and a `&mut &mut` in a struct field derefs twice to its cell.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_multi_level_reference_aliasing() {
    let result =
        interpret_fixture("interp_refs_double_deref_alias").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interprets_program_with_inputs() {
    let program_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../noir/test_programs/execution_success/assert_statement");
    if !program_dir.is_dir() {
        eprintln!("SKIPPED (vacuous pass): noir corpus not checked out at ../noir");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("pkg");
    copy_dir(&program_dir, &root);

    let project = NoirProject::new(root.clone()).unwrap();
    let validated = compile_for_validation(&project).unwrap();
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();
    let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml).unwrap();

    let result = interpret_with_inputs(&validated.program, inputs).unwrap();
    assert_eq!(result, Value::Unit);
}

/// Differential correctness: the interpreter's computed return value matches the expected output
/// Noir's corpus records in `Prover.toml`. `arithmetic_binary_operations` returns 10 (a u64),
/// so this verifies the actual value, not merely that interpretation didn't error.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn interpreter_return_matches_recorded_expected() {
    let program_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../noir/test_programs/execution_success/arithmetic_binary_operations");
    if !program_dir.is_dir() {
        eprintln!("SKIPPED (vacuous pass): noir corpus not checked out at ../noir");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("pkg");
    copy_dir(&program_dir, &root);

    let project = NoirProject::new(root.clone()).unwrap();
    let validated = compile_for_validation(&project).unwrap();
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();

    let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml).unwrap();
    let value = interpret_with_inputs(&validated.program, inputs).unwrap();
    let expected = expected_return_from_prover_toml(&validated.program, &validated.abi, &toml)
        .unwrap()
        .expect("this program records a return value");

    assert_eq!(
        value, expected,
        "interpreter output must match Noir's recorded return"
    );
}

/// A `u64` program whose constant exceeds the Goldilocks modulus (`p + 1`) compiles to a mono-AST
/// under goldilocks via the reachability-only frontend, and the interpreter computes the correct
/// native `u64` result — proving the Goldilocks frontend did not corrupt the integer. The expected
/// value is field-independent, so this single assertion is the cross-field equivalence claim for
/// this program (the corpus-wide version is `dump_corpus_outcomes` + `cross_field_diff`).
#[cfg(feature = "goldilocks")]
#[test]
fn validates_goldilocks_mono_ast_u64() {
    let project = NoirProject::new(fixture("interp_inputs_u64")).expect("project");
    let validated = compile_for_validation(&project)
        .expect("goldilocks frontend should produce a mono-AST for a stdlib-free u64 program");

    // main(x: u64) -> u64 = x * 2 + (p + 1). With x = 3: 6 + 18446744069414584322.
    let x = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(3u64),
    });
    let result = interpret_with_inputs(&validated.program, vec![x]).expect("interpret");
    let expected = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(18446744069414584328u64),
    });
    assert_eq!(
        result, expected,
        "Goldilocks mono-AST must carry p+1 exactly and compute the native u64 result"
    );
}

// --- Shared corpus machinery for the coverage survey and cross-field differential. ---

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

/// Compile a corpus program through Noir's frontend + monomorphizer and interpret it under a panic
/// guard, returning the value or a stage-tagged failure. The single interpret path shared by the
/// coverage survey and the cross-field dump.
fn compile_and_interpret_caught(program_dir: &Path) -> Result<Value, (FailureKind, String)> {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("pkg");
    copy_dir(program_dir, &root);

    panic::catch_unwind(AssertUnwindSafe(|| {
        let project = NoirProject::new(root.clone())
            .map_err(|e| (FailureKind::ProjectLoad, e.to_string()))?;
        // The reachability-only validation frontend does not let the unreached bn254 crypto stdlib
        // block the Goldilocks arm.
        let validated = compile_for_validation(&project)
            .map_err(|e| (validation_failure_kind(&e), e.to_string()))?;
        let inputs = match std::fs::read_to_string(root.join("Prover.toml")) {
            Ok(src) => inputs_from_prover_toml(&validated.program, &validated.abi, &src)
                .map_err(|e| (failure_kind_of(&e), e.to_string()))?,
            Err(_) => Vec::new(),
        };
        interpret_with_inputs(&validated.program, inputs)
            .map_err(|e| (failure_kind_of(&e), e.to_string()))
    }))
    .unwrap_or_else(|payload| Err((FailureKind::Panic, panic_message(&payload))))
}

/// The first non-empty line of a caught panic payload, for triage.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .map(|m| m.lines().next().unwrap_or("").trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "panic".to_string())
}

/// One program's coverage bucket for the survey: the outcome kind, or the unsupported construct, so
/// the report maps which constructs block coverage.
#[cfg(not(feature = "goldilocks"))]
fn classify(program_dir: &Path) -> String {
    match compile_and_interpret_caught(program_dir) {
        Ok(_) => "returned a value".to_string(),
        Err((FailureKind::Unsupported { construct }, _)) => format!("unsupported: {construct}"),
        Err((kind, _)) => format!("{kind:?}"),
    }
}

/// One program's serializable, field-independent outcome for the cross-field differential.
fn run_outcome(program_dir: &Path) -> DiffOutcome {
    match compile_and_interpret_caught(program_dir) {
        Ok(value) => DiffOutcome::Returned(DiffValue::from_value(&value)),
        Err((kind, detail)) => DiffOutcome::Errored { kind, detail },
    }
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../noir/test_programs/execution_success")
}

/// The field this build targets, used to tag the dump file. The two fields are separate builds
/// (the field is selected at compile time), so each writes its own file for the diff to consume.
fn field_tag() -> &'static str {
    if cfg!(feature = "goldilocks") {
        "goldilocks"
    } else {
        "bn254"
    }
}

fn target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target")
}

fn dump_path(tag: &str) -> PathBuf {
    target_dir().join(format!("cross_field_{tag}.json"))
}

/// `git rev-parse HEAD` in `dir`, or "unknown" when git is unavailable.
fn git_rev(dir: &str) -> String {
    std::process::Command::new("git")
        .args(["-C", dir, "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The pinned noir rev, read from the `noirc_frontend` git source in `Cargo.lock`. Returns
/// "unknown" when a local `[patch]` shadows it with a path source, so a patched dev build is never
/// mistaken for a pinned one.
fn noir_rev() -> String {
    let lock = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    let Ok(text) = std::fs::read_to_string(lock) else {
        return "unknown".to_string();
    };
    let mut in_frontend = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "name = \"noirc_frontend\"" {
            in_frontend = true;
        } else if in_frontend {
            if let Some(source) = line.strip_prefix("source = \"") {
                return source
                    .rsplit_once('#')
                    .map(|(_, rev)| rev.trim_end_matches('"').to_string())
                    .unwrap_or_else(|| "unknown".to_string());
            }
            if line.starts_with("name = ") {
                break; // left the package block with no git source (path patch active)
            }
        }
    }
    "unknown".to_string()
}

/// Cross-field differential, step 1: dump this build's per-program outcomes plus provenance to a
/// field-tagged JSON file. Run once per field:
///   cargo test ... --lib tests::dump_corpus_outcomes -- --ignored
///   cargo test ... --features goldilocks --lib tests::dump_corpus_outcomes -- --ignored
/// Dependency compilation gaps are recorded as errored outcomes.
#[test]
#[ignore = "cross-field differential: dump this field's interpreter outcomes"]
fn dump_corpus_outcomes() {
    let corpus = corpus_dir();
    assert!(corpus.is_dir(), "corpus not found at {}", corpus.display());
    std::fs::create_dir_all(target_dir()).expect("create target dir for the dump");

    let mut outcomes: Vec<(String, DiffOutcome)> = Vec::new();
    for entry in std::fs::read_dir(&corpus).unwrap() {
        let dir = entry.unwrap().path();
        let manifest = dir.join("Nargo.toml");
        if !dir.is_dir() || !manifest.exists() {
            continue;
        }
        if std::fs::read_to_string(&manifest)
            .map(|s| s.contains("[workspace]"))
            .unwrap_or(false)
        {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        outcomes.push((name, run_outcome(&dir)));
    }

    let returned = outcomes
        .iter()
        .filter(|(_, o)| matches!(o, DiffOutcome::Returned(_)))
        .count();
    let dump = CrossFieldDump {
        provenance: DumpProvenance {
            format_version: DUMP_FORMAT_VERSION,
            field: field_tag().to_string(),
            field_modulus: FieldElement::modulus().to_string(),
            noir_rev: noir_rev(),
            interpreter_rev: git_rev(env!("CARGO_MANIFEST_DIR")),
            corpus_dir: corpus.to_string_lossy().into_owned(),
            program_count: outcomes.len(),
            built_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
        },
        outcomes,
    };

    // Round-trip through JSON to validate serialization before writing.
    let json = serde_json::to_string_pretty(&dump).unwrap();
    let restored: CrossFieldDump = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, dump, "dump must round-trip through JSON");

    let path = dump_path(field_tag());
    std::fs::write(&path, &json)
        .unwrap_or_else(|e| panic!("failed to write the dump to {}: {e}", path.display()));
    println!(
        "wrote {} outcomes ({returned} returned a value) for field '{}' to {}",
        dump.outcomes.len(),
        field_tag(),
        path.display()
    );
}

/// Programs whose result is legitimately field-dependent, with a justification each. Entries are
/// excluded from the divergence gate but still printed so they stay visible.
const KNOWN_FIELD_DEPENDENT: &[(&str, &str)] = &[];

/// The divergence class for a mismatched outcome pair, for the bucketed report.
fn divergence_bucket(a: &DiffOutcome, b: &DiffOutcome) -> &'static str {
    match (a, b) {
        (DiffOutcome::Returned(_), DiffOutcome::Returned(_)) => "value_mismatch",
        (DiffOutcome::Returned(_), DiffOutcome::Errored { .. })
        | (DiffOutcome::Errored { .. }, DiffOutcome::Returned(_)) => "returned_vs_errored",
        (DiffOutcome::Errored { kind: ka, .. }, DiffOutcome::Errored { kind: kb, .. }) => {
            if matches!(ka, FailureKind::Panic) || matches!(kb, FailureKind::Panic) {
                "panic"
            } else {
                "kind_mismatch"
            }
        }
    }
}

/// Write the bucketed divergence report to `target/cross_field_report.{json,md}`.
fn write_cross_field_report(
    divergences: &[(String, &'static str, String)],
    tolerated: &[String],
    missing: &[String],
) {
    use std::collections::BTreeMap;
    let dir = target_dir();
    std::fs::create_dir_all(&dir).expect("create target dir for the report");
    std::fs::write(
        dir.join("cross_field_report.json"),
        serde_json::to_string_pretty(divergences).expect("serialize cross-field divergences"),
    )
    .expect("write cross_field_report.json");

    let mut by_bucket: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (name, bucket, reason) in divergences {
        by_bucket
            .entry(bucket)
            .or_default()
            .push(format!("{name}: {reason}"));
    }
    let mut md = String::from("# Cross-field divergence report\n\n");
    md.push_str(&format!(
        "- tolerated (coverage gap on a side): {}\n- missing from a dump: {}\n\n",
        tolerated.len(),
        missing.len()
    ));
    if !tolerated.is_empty() {
        md.push_str(&format!("## tolerated ({})\n\n", tolerated.len()));
        for name in tolerated {
            md.push_str(&format!("- {name}\n"));
        }
        md.push('\n');
    }
    for (bucket, entries) in &by_bucket {
        md.push_str(&format!("## {bucket} ({})\n\n", entries.len()));
        for entry in entries {
            md.push_str(&format!("- {entry}\n"));
        }
        md.push('\n');
    }
    std::fs::write(dir.join("cross_field_report.md"), md).expect("write cross_field_report.md");
}

/// Cross-field differential, step 2: diff the two field dumps. Integer/struct values must match;
/// `Field` values may differ. Refuses to run when the dumps come from mismatched builds, tolerates
/// coverage gaps (counted separately) while never hiding a crash, and writes a
/// bucketed report to `target/cross_field_report.{json,md}`.
#[test]
#[ignore = "cross-field differential: diff the bn254 and goldilocks dumps"]
fn cross_field_diff() {
    use std::collections::{BTreeMap, HashSet};

    let load = |tag: &str| -> Option<CrossFieldDump> {
        let text = std::fs::read_to_string(dump_path(tag)).ok()?;
        Some(serde_json::from_str(&text).unwrap_or_else(|e| {
            panic!("the {tag} dump did not parse (stale format? re-run dump_corpus_outcomes under both fields): {e}")
        }))
    };
    let (Some(bn254), Some(goldilocks)) = (load("bn254"), load("goldilocks")) else {
        panic!("missing a dump; run dump_corpus_outcomes under each field first");
    };

    // Provenance gate: refuse mismatched builds rather than invent divergences from them.
    let (pa, pb) = (&bn254.provenance, &goldilocks.provenance);
    for (p, tag) in [(pa, "bn254"), (pb, "goldilocks")] {
        assert_eq!(
            p.format_version, DUMP_FORMAT_VERSION,
            "the {tag} dump format is stale; re-run dump_corpus_outcomes under both fields"
        );
    }
    assert_ne!(
        pa.field_modulus, pb.field_modulus,
        "both dumps are the same field ({}); nothing to diff",
        pa.field
    );
    assert_eq!(
        pa.noir_rev, pb.noir_rev,
        "dumps built against different noir revs: {} vs {}",
        pa.noir_rev, pb.noir_rev
    );
    assert_eq!(
        pa.interpreter_rev, pb.interpreter_rev,
        "dumps built against different interpreter revs: {} vs {}",
        pa.interpreter_rev, pb.interpreter_rev
    );
    assert_eq!(
        pa.corpus_dir, pb.corpus_dir,
        "dumps built against different corpora"
    );
    assert_eq!(
        pa.program_count, pb.program_count,
        "dumps cover a different program count: {} vs {}",
        pa.program_count, pb.program_count
    );

    let bn: BTreeMap<_, _> = bn254.outcomes.into_iter().collect();
    let gl: BTreeMap<_, _> = goldilocks.outcomes.into_iter().collect();
    let allowlisted: HashSet<&str> = KNOWN_FIELD_DEPENDENT
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let mut divergences: Vec<(String, &'static str, String)> = Vec::new();
    let mut allowlisted_hits: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut tolerated_names: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut value_pairs = 0usize;

    for (name, b) in &bn {
        let Some(g) = gl.get(name) else {
            missing.push(format!("{name} (only in bn254)"));
            continue;
        };
        compared += 1;
        if matches!((b, g), (DiffOutcome::Returned(_), DiffOutcome::Returned(_))) {
            value_pairs += 1;
        }
        match outcomes_equivalent(b, g) {
            Ok(()) => {
                if outcome_is_tolerated(b, g) {
                    tolerated_names.push(name.clone());
                }
            }
            Err(reason) if allowlisted.contains(name.as_str()) => {
                allowlisted_hits.push(format!("{name}: {reason}"));
            }
            Err(reason) => divergences.push((name.clone(), divergence_bucket(b, g), reason)),
        }
    }
    for name in gl.keys() {
        if !bn.contains_key(name) {
            missing.push(format!("{name} (only in goldilocks)"));
        }
    }

    let tolerated = tolerated_names.len();
    write_cross_field_report(&divergences, &tolerated_names, &missing);

    println!(
        "compared {compared} programs: {} divergent, {tolerated} tolerated (coverage gap), {} allowlisted, {} missing",
        divergences.len(),
        allowlisted_hits.len(),
        missing.len()
    );
    for (name, bucket, reason) in &divergences {
        println!("  DIVERGENCE [{bucket}] {name}: {reason}");
    }
    for hit in &allowlisted_hits {
        println!("  allowlisted (field-dependent): {hit}");
    }
    assert!(
        divergences.is_empty(),
        "{} cross-field divergence(s) outside the allowlist",
        divergences.len()
    );
    assert!(
        missing.is_empty(),
        "{} program(s) were missing from one field dump: {}",
        missing.len(),
        missing.join(", ")
    );
    assert!(
        value_pairs > 0,
        "none of the {compared} shared programs returned values under both fields"
    );
}

#[cfg(not(feature = "goldilocks"))]
#[test]
#[ignore = "manual coverage survey over Noir's execution_success corpus"]
fn survey_execution_success_corpus() {
    use std::collections::BTreeMap;

    let corpus =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../noir/test_programs/execution_success");
    assert!(corpus.is_dir(), "corpus not found at {}", corpus.display());

    let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total = 0;
    for entry in std::fs::read_dir(&corpus).unwrap() {
        let dir = entry.unwrap().path();
        let manifest = dir.join("Nargo.toml");
        if !dir.is_dir() || !manifest.exists() {
            continue;
        }
        // Workspaces have multiple members; the driver picks one arbitrarily, which doesn't match
        // the workspace's default-member `main`, so skip them rather than report a false failure.
        if std::fs::read_to_string(&manifest)
            .map(|s| s.contains("[workspace]"))
            .unwrap_or(false)
        {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        total += 1;
        buckets.entry(classify(&dir)).or_default().push(name);
    }

    println!("\n=== interpreter coverage over {total} execution_success programs ===");
    for (bucket, names) in &buckets {
        println!("\n[{}]  {}", names.len(), bucket);
        for name in names {
            println!("    {name}");
        }
    }
}

// --- Differential oracle: interpreter vs Noir's own ACVM/Brillig executor (see `noir_oracle.rs`).
// Two independent lowerings (tree-walk vs full ACIR compile+execute) must agree on the return. ---

/// Run one program through both the interpreter and Noir's executor and classify the comparison.
/// The executor runs even when the interpreter rejects the program, so a *false rejection* (interp
/// errors on something nargo runs fine) is caught, not hidden. Buckets: `"agree"`,
/// `"FALSE-REJECTION: ..."`, `"MISMATCH: ..."`, `"oracle-wrong: ..."`, `"interp-unsupported: ..."`
/// (tolerated gap), `"oracle-errored"`, `"both-errored"`. Under goldilocks the executor can't
/// elaborate the bn254 stdlib, so comparisons stay vacuous.
fn oracle_compare(program_dir: &Path) -> String {
    use super::noir_oracle::noir_execute_return;

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("pkg");
    copy_dir(program_dir, &root);
    let prover_src = std::fs::read_to_string(root.join("Prover.toml")).ok();

    let project = match panic::catch_unwind(AssertUnwindSafe(|| NoirProject::new(root.clone()))) {
        Ok(Ok(p)) => p,
        _ => return "project-errored".to_string(),
    };

    // The executor runs independently (its own compile+execute), so it can succeed on a program the
    // interpreter's frontend wrongly rejects. `Some(ret)` = executor succeeded; `None` = it errored.
    let oracle = panic::catch_unwind(AssertUnwindSafe(|| {
        noir_execute_return(&project, prover_src.as_deref())
    }));
    let executor_ok = match oracle {
        Ok(Ok(ret)) => Some(ret),
        Ok(Err(_)) | Err(_) => None,
    };

    // Interp side: frontend-compile + interpret, both caught; keep `validated` to decode the
    // executor's return when both succeed.
    let interp = panic::catch_unwind(AssertUnwindSafe(|| {
        let validated = compile_for_validation(&project)
            .map_err(|e| (validation_failure_kind(&e), e.to_string()))?;
        let inputs = match &prover_src {
            Some(src) => inputs_from_prover_toml(&validated.program, &validated.abi, src)
                .map_err(|e| (failure_kind_of(&e), e.to_string()))?,
            None => Vec::new(),
        };
        let value = interpret_with_inputs(&validated.program, inputs)
            .map_err(|e| (failure_kind_of(&e), e.to_string()))?;
        Ok::<_, (FailureKind, String)>((validated, value))
    }));
    let interp = match interp {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(kd)) => Err(kd),
        Err(payload) => Err((FailureKind::Panic, panic_message(&payload))),
    };

    match (interp, executor_ok) {
        (Err((FailureKind::Unsupported { construct }, _)), Some(_)) => {
            format!("interp-unsupported: {construct}")
        }
        (Err((kind, detail)), Some(_)) => {
            format!("FALSE-REJECTION: interp {kind:?} ({detail}) but executor ran")
        }
        (Err(_), None) => "both-errored".to_string(),
        (Ok(_), None) => "oracle-errored".to_string(),
        (Ok((validated, interp_value)), Some(oracle_ret)) => {
            // Decode the executor's return using the interpreter's mono return type, then compare
            // exactly (same field — `Field` values must match too, unlike the cross-field diff).
            let oracle_value = match oracle_ret {
                None => Value::Unit,
                Some(iv) => {
                    let ret_ty = match crate::main_function_of(&validated.program) {
                        Ok(f) => &f.return_type,
                        Err(e) => return format!("oracle-errored: {e}"),
                    };
                    match validated.abi.return_type.as_ref() {
                        Some(r) => match crate::input::value_from_input(&iv, &r.abi_type, ret_ty) {
                            Ok(v) => v,
                            Err(e) => return format!("oracle-errored: decode: {e}"),
                        },
                        None => return "oracle-errored: return with no ABI type".to_string(),
                    }
                }
            };
            if interp_value == oracle_value {
                return "agree".to_string();
            }
            // Adjudicate a disagreement with the corpus's recorded `return` (Noir's ground truth).
            // `compile_main` is a secondary oracle and can itself be wrong on edge cases, so an
            // interpreter that matches ground truth while the executor doesn't is an oracle
            // limitation, bucketed apart so it doesn't fail the gate.
            let recorded = prover_src.as_deref().and_then(|src| {
                super::expected_return_from_prover_toml(&validated.program, &validated.abi, src)
                    .ok()
                    .flatten()
            });
            match recorded {
                Some(gt) if interp_value == gt && oracle_value != gt => {
                    format!(
                        "oracle-wrong: interp={interp_value:?} matches recorded, oracle={oracle_value:?}"
                    )
                }
                _ => format!("MISMATCH: interp={interp_value:?} oracle={oracle_value:?}"),
            }
        }
    }
}

/// The interpreter and Noir's ACVM executor agree on the in-crate fixtures. bn254 only — under
/// goldilocks the executor cannot compile them yet.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn oracle_matches_interpreter_smoke() {
    // interp_inputs_mixed is left out: its shape trips Noir's ACIR flattening pass, so the executor
    // cannot judge it (the interpreter still runs it — see `interprets_mixed_inputs`).
    for name in [
        "interp_basic",
        "interp_inputs_u64",
        "interp_inputs_i32",
        "interp_inputs_struct",
    ] {
        let result = oracle_compare(&fixture(name));
        assert_eq!(
            result, "agree",
            "interpreter/executor disagreement on {name}: {result}"
        );
    }
}

/// The real correctness gate: run the whole `execution_success` corpus through both the interpreter
/// and Noir's executor and fail on any `MISMATCH` (a genuine interpreter bug) or `FALSE-REJECTION`
/// (the interpreter rejected a program the executor ran fine). Tolerated `interp-unsupported` cases
/// are counted, not failed, so a coverage gap stays visible without hiding a false rejection.
/// `#[ignore]`d (slow) and needs a big stack (deep programs):
///   RUST_MIN_STACK=1073741824 cargo test --lib \
///       tests::oracle_survey_execution_success -- --ignored --nocapture
#[test]
#[ignore = "differential oracle: interpreter vs Noir's ACVM executor over the corpus"]
fn oracle_survey_execution_success() {
    use std::collections::BTreeMap;

    let corpus = corpus_dir();
    assert!(corpus.is_dir(), "corpus not found at {}", corpus.display());

    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut total = 0;
    for entry in std::fs::read_dir(&corpus).unwrap() {
        let dir = entry.unwrap().path();
        let manifest = dir.join("Nargo.toml");
        if !dir.is_dir() || !manifest.exists() {
            continue;
        }
        if std::fs::read_to_string(&manifest)
            .map(|s| s.contains("[workspace]"))
            .unwrap_or(false)
        {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        total += 1;
        let result = oracle_compare(&dir);
        let bucket = result
            .split(':')
            .next()
            .unwrap_or(&result)
            .trim()
            .to_string();
        *buckets.entry(bucket).or_default() += 1;
        if result.starts_with("MISMATCH") || result.starts_with("FALSE-REJECTION") {
            failures.push(format!("{name}: {result}"));
        }
    }

    let tolerated = buckets.get("interp-unsupported").copied().unwrap_or(0);
    println!("\n=== interpreter vs Noir-executor over {total} execution_success programs ===");
    for (bucket, count) in &buckets {
        println!("  {count:4}  {bucket}");
    }
    println!("  ({tolerated} tolerated interp-unsupported — a measured coverage gap, not a pass)");
    for failure in &failures {
        println!("  {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} interpreter/executor failure(s) found (MISMATCH or FALSE-REJECTION)",
        failures.len()
    );
}

// Parked behind an always-false cfg until the mavros-compiler dependency is available; restore
// `#[cfg(all(feature = "mavros-oracle", not(feature = "goldilocks")))]` then.
#[cfg(any())]
mod mavros_oracle {
    use super::{
        NoirProject, Value, compile_for_validation, fixture, inputs_from_prover_toml, interpret,
        interpret_with_inputs,
    };
    use mavros_compiler::{driver::Driver, project::Project};

    /// The integration driver and the pure-Noir frontend should agree on a stdlib-free fixture.
    #[test]
    fn integration_driver_agrees_with_pure_noir() {
        let root = fixture("interp_inputs_u64");
        let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");

        // pure-Noir side
        let noir = NoirProject::new(root.clone()).expect("noir project");
        let validated = compile_for_validation(&noir).expect("pure-noir frontend");
        let noir_inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml)
            .expect("noir inputs");
        let noir_result =
            interpret_with_inputs(&validated.program, noir_inputs).expect("noir interpret");

        // Integration side.
        let project = Project::new(root.clone()).expect("oracle project");
        let mut driver = Driver::new(project, false);
        driver.run_noir_compiler().expect("oracle compile");
        let oracle_program = driver.monomorphized_program();
        let oracle_inputs =
            inputs_from_prover_toml(oracle_program, driver.abi(), &toml).expect("oracle inputs");
        let oracle_result =
            interpret_with_inputs(oracle_program, oracle_inputs).expect("oracle interpret");

        assert_eq!(
            noir_result, oracle_result,
            "integration AST must interpret identically to pure-Noir"
        );
    }

    /// Exercises the `PackageSource` impl used by the optional oracle.
    #[test]
    fn compile_for_validation_accepts_oracle_project() {
        let project = Project::new(fixture("interp_basic")).expect("oracle project");
        let validated = compile_for_validation(&project).expect("validate oracle project");
        let result = interpret(&validated.program).expect("interpret");
        assert_eq!(result, Value::Unit);
    }
}
