use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use super::corpus::{compile_error_of, copy_dir, corpus_dir, list_programs, panic_message};
use super::diff::{FailureKind, failure_kind_of};
use super::loader::NoirProject;
use super::validation_frontend::compile_for_validation;
use super::{IntValue, InterpretError, Value, inputs_from_prover_toml, interpret_with_inputs};
#[cfg(not(feature = "goldilocks"))]
use super::{expected_return_from_prover_toml, interpret};
use num_bigint::BigInt;

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

/// A false (non-const-folded) assertion interprets to `AssertionFailed`, not a clean pass.
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

/// A type error in code `main` *reaches* must be rejected, not silently monomorphized. A clean
/// `Ok(())` is the hole we guard against; a monomorphizer panic on the `Error` node also counts.
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

/// The `Prover.toml` input bridge: `interp_inputs_u64` with `x = 3` computes `x*2 + (p+1)` in u64.
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

/// Signed i32 inputs decode identically on both fields and drive signed arithmetic.
/// `a = -7, b = 2` → `-121`.
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

/// bn254 i64 control: with 2^64 < p the encoding is injective, so `x = -1` decodes correctly.
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

/// Under Goldilocks no i64 input is silently decoded: a negative exceeds the modulus, and an
/// in-field positive is refused by the representability guard (i64's 2^64 range exceeds the field).
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
/// `zeta*1000 + alpha*100 + (1+2+3) == 3706`.
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

/// Array input, helper call, indexed loop, and a signed conditional, all from `Prover.toml`.
/// `xs=[10,20,30,40]` (weighted `300`), `k=-3` (negative branch) → `300 - 5 == 295`.
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

/// An enum `match` binds a variant's payload via the `(tag, payload…)` tuple. `x = 3` → `3 * 4 == 12`.
#[test]
fn interprets_enum_match() {
    let root = fixture("interp_match_enum");
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
            value: BigInt::from(12)
        })
    );
}

/// A literal-integer `match`: an exact case (`x = 2 => 300`), the wildcard `default_case`
/// (`x = 5 => 50`), and a negative signed literal case (`x = -2 => 100`).
#[test]
fn interprets_integer_match() {
    let validated = {
        let project = NoirProject::new(fixture("interp_match_int")).expect("project");
        compile_for_validation(&project).expect("frontend")
    };
    let run = |x: i32| {
        let input = Value::Int(IntValue {
            signed: true,
            bits: 32,
            value: BigInt::from(x),
        });
        interpret_with_inputs(&validated.program, vec![input]).expect("interpret")
    };
    let i32v = |v: i32| {
        Value::Int(IntValue {
            signed: true,
            bits: 32,
            value: BigInt::from(v),
        })
    };
    assert_eq!(run(2), i32v(300), "exact case");
    assert_eq!(run(5), i32v(50), "wildcard default (5 * 10)");
    assert_eq!(run(-2), i32v(100), "negative literal case");
}

#[cfg(not(feature = "goldilocks"))]
#[test]
fn renders_assert_message() {
    let project = NoirProject::new(negative_fixture("assert_fmt_msg")).expect("project");
    let validated = compile_for_validation(&project).expect("frontend compile");
    match interpret(&validated.program) {
        Err(InterpretError::AssertionFailed {
            message: Some(m), ..
        }) => assert_eq!(
            m,
            "sum=45 field=0x03 array=[1, 2] tuple=(7,) point=Point { x: 4, y: true } choice=Choice::Some(9)"
        ),
        other => panic!("expected AssertionFailed with a rendered message, got {other:?}"),
    }
}

/// A `main` with inputs interprets correctly from `Prover.toml`. `assert_statement` has `x == y == 3`.
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

/// A `u64` program whose constant exceeds the Goldilocks modulus (`p + 1`) compiles under goldilocks
/// and computes the correct native `u64` result, proving the frontend did not corrupt the integer.
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

// --- Differential oracle: interpreter vs Noir's own ACVM/Brillig executor (see `noir_oracle.rs`).
// Two independent lowerings (tree-walk vs full ACIR compile+execute) must agree on the return. ---

/// Run one program through both the interpreter and Noir's executor and classify the comparison.
/// The executor runs even when the interpreter rejects the program, so a *false rejection* (interp
/// errors on something nargo runs fine) is caught, not hidden. Buckets: `"agree"`,
/// `"FALSE-REJECTION: ..."`, `"MISMATCH: ..."`, `"oracle-wrong: ..."`, `"interp-unsupported: ..."`
/// (tolerated gap), `"interp-panic: ..."` (always an interpreter bug, never folded into
/// `both-errored`), `"oracle-errored"`, `"both-errored"`. Under goldilocks the executor can't
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
            .map_err(|e| (compile_error_of(&e).kind, e.to_string()))?;
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
        Err(payload) => Err((FailureKind::Panic, panic_message(payload.as_ref()))),
    };

    match (interp, executor_ok) {
        (Err((FailureKind::Panic, detail)), _) => format!("interp-panic: {detail}"),
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
        "interp_refs_struct_field",
        "interp_refs_call_chain",
        "interp_refs_nested_field",
        "interp_refs_double_deref_alias",
        "interp_match_enum",
        "interp_match_int",
        "intrinsic_slice_ops",
        "intrinsic_conversions",
        "intrinsic_to_bytes",
        "intrinsic_range_constraint",
        "interp_intrinsic_hints",
        "interp_aggregate_eq",
        "interp_closures",
    ] {
        let result = oracle_compare(&fixture(name));
        assert_eq!(
            result, "agree",
            "interpreter/executor disagreement on {name}: {result}"
        );
    }
}

/// The real correctness gate: run the whole `execution_success` corpus through the interpreter and
/// Noir's executor and fail on any `MISMATCH` or `FALSE-REJECTION`. Tolerated `interp-unsupported`
/// is counted, not failed. `#[ignore]`d and needs a big stack:
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
    for program in list_programs(&corpus).iter().filter(|p| !p.workspace) {
        let name = &program.name;
        total += 1;
        let result = oracle_compare(&program.dir);
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
