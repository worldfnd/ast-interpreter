use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::corpus::{
    compile_error_of, copy_dir, corpus_dir, list_programs, panic_message, temp_noir_package,
};
use super::diff::{FailureKind, comparable_error_of};
use super::expected_return_from_prover_toml;
use super::loader::NoirProject;
use super::validation_frontend::{Validated, compile_for_validation, stdlib_tests};
use super::{
    IntValue, InterpretError, Value, inputs_from_prover_toml, interpret, interpret_with_inputs,
};
use acvm::{FieldConfig, FieldId, FieldValue};
use noirc_frontend::token::TestScope;
use num_bigint::BigInt;

/// A test Noir package under `fixtures/`. Positive packages keep a plain name; negatives carry a
/// `neg_` prefix (built via [`negative_fixture`]).
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// A `neg_`-prefixed fixture: a program expected to fail to compile or assert.
fn negative_fixture(name: &str) -> PathBuf {
    fixture(&format!("neg_{name}"))
}

/// Compile a fixture through Noir's frontend + monomorphizer and interpret the resulting AST.
fn interpret_fixture(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let project = NoirProject::new(fixture(name))?;
    let validated = compile_for_validation(&project, FieldId::linked())?;
    Ok(interpret(&validated.program, validated.field_id)?)
}

fn assert_fixture_return(name: &str, expected: Value) {
    let root = fixture(name);
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked())
        .unwrap_or_else(|error| panic!("{name}: frontend: {error}"));
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap_or_else(|error| panic!("{name}: inputs: {error}"));
    let result = interpret_with_inputs(&validated.program, inputs, validated.field_id)
        .unwrap_or_else(|error| panic!("{name}: interpret: {error}"));
    let recorded = expected_return_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap_or_else(|error| panic!("{name}: recorded return: {error}"));
    assert_eq!(result, expected, "{name}");
    assert_eq!(recorded, Some(expected), "{name}: recorded return");
}

fn compile_source(source: &str, field: FieldId) -> Validated {
    let root = temp_noir_package("test", source);
    let project = NoirProject::new(root.path().to_path_buf()).expect("project");
    compile_for_validation(&project, field).expect("frontend compile")
}

#[test]
fn one_build_interprets_a_program_under_two_fields() {
    let project = NoirProject::new(fixture("interp_basic")).expect("project");
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_for_validation(&project, field)
            .unwrap_or_else(|error| panic!("{field}: frontend compile: {error:?}"));
        assert_eq!(
            validated.field_id, field,
            "the output carries the field asked for"
        );
        let result = interpret(&validated.program, validated.field_id)
            .unwrap_or_else(|error| panic!("{field}: interpret: {error:?}"));
        assert_eq!(result, Value::Unit, "{field}: main returns unit");

        let validated = compile_source("fn main(x: Field) -> pub Field { x - 2 }", field);
        let result = interpret_with_inputs(
            &validated.program,
            vec![Value::Field(FieldValue::one(field))],
            validated.field_id,
        )
        .expect("field arithmetic");
        let Value::Field(result) = result else {
            panic!("expected a Field return")
        };
        assert_eq!(result.field(), field);
        assert_eq!(
            result.as_biguint(),
            &(FieldConfig::new(field).modulus() - 1u8)
        );
    }
}

#[test]
fn toml_bridge_requires_the_linked_field() {
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_source("fn main(x: Field) -> pub Field { x }", field);
        for (toml, expected) in [
            ("x = -1\nreturn = -1", -FieldValue::one(field)),
            ("x = 1\nreturn = 1", FieldValue::one(field)),
        ] {
            let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, toml, field);
            let recorded =
                expected_return_from_prover_toml(&validated.program, &validated.abi, toml, field);
            if field == FieldId::linked() {
                assert_eq!(inputs.unwrap(), vec![Value::Field(expected.clone())]);
                assert_eq!(recorded.unwrap(), Some(Value::Field(expected)));
            } else {
                assert!(
                    matches!(inputs, Err(InterpretError::InvalidInput(_))),
                    "{field}: {inputs:?}"
                );
                assert!(
                    matches!(recorded, Err(InterpretError::InvalidInput(_))),
                    "{field}: {recorded:?}"
                );
            }
        }
    }
}

#[test]
fn rejects_inputs_from_another_field() {
    let field = FieldId::linked();
    let other = if field == FieldId::Bn254 {
        FieldId::Goldilocks
    } else {
        FieldId::Bn254
    };
    let good = Value::Field(FieldValue::one(field));
    let bad = Value::Field(FieldValue::one(other));
    let cell = |value: &Value| Rc::new(RefCell::new(value.clone()));
    for (source, good_input, bad_input) in [
        (
            "fn main(x: Field) -> pub Field { x }",
            good.clone(),
            bad.clone(),
        ),
        (
            "fn main(x: ([Field; 1], Field)) -> pub Field { x.1 }",
            Value::tuple(vec![Value::Array(vec![good.clone()]), good.clone()]),
            Value::tuple(vec![Value::Array(vec![bad.clone()]), good.clone()]),
        ),
    ] {
        let validated = compile_source(source, field);
        assert_eq!(
            interpret_with_inputs(&validated.program, vec![good_input], field).unwrap(),
            good
        );
        let result = interpret_with_inputs(&validated.program, vec![bad_input], field);
        assert!(
            matches!(result, Err(InterpretError::InvalidInput(_))),
            "{result:?}"
        );
    }

    // Noir refuses a reference as an entry-point type, so a `Ref` input is always a caller error
    // and has no accepted spelling to check; the traversal still has to look inside one rather
    // than take it for a leaf.
    let validated = compile_source("fn main(x: Field) -> pub Field { x }", field);
    for hidden in [
        Value::Ref(cell(&bad), false),
        Value::Array(vec![Value::Ref(cell(&bad), false)]),
        Value::tuple(vec![Value::Ref(cell(&bad), true)]),
    ] {
        match interpret_with_inputs(&validated.program, vec![hidden.clone()], field) {
            Err(InterpretError::InvalidInput(message)) => {
                assert!(
                    message.contains(&format!("belongs to {other}")),
                    "{message}"
                )
            }
            result => panic!("{hidden:?}: {result:?}"),
        }
    }
}

/// `IntValue`'s members are public, so an input can name a value its own type does not hold.
#[test]
fn rejects_an_integer_input_outside_its_type() {
    let field = FieldId::linked();
    let int = |signed, bits, value: i64| IntValue {
        signed,
        bits,
        value: BigInt::from(value),
    };
    let validated = compile_source("fn main(x: [u8; 1], y: i8) -> pub u8 { x[0] }", field);
    let run = |x: IntValue, y: IntValue| {
        let inputs = vec![Value::Array(vec![Value::Int(x)]), Value::Int(y)];
        interpret_with_inputs(&validated.program, inputs, field)
    };
    let (u8_max, i8_min) = (int(false, 8, 255), int(true, 8, -128));
    assert_eq!(
        run(u8_max.clone(), i8_min.clone()).unwrap(),
        Value::Int(int(false, 8, 255))
    );
    for (x, y) in [
        (int(false, 8, 256), i8_min.clone()),
        (int(false, 8, -1), i8_min.clone()),
        (int(false, 0, 0), i8_min.clone()),
        (u8_max.clone(), int(true, 8, 128)),
        (u8_max.clone(), int(true, 8, -129)),
    ] {
        let result = run(x.clone(), y.clone());
        assert!(
            matches!(result, Err(InterpretError::InvalidInput(_))),
            "{x:?}, {y:?}: {result:?}"
        );
    }
}

/// Integer-to-integer casts keep a `u64` above the Goldilocks modulus intact, at run time and in
/// a `comptime` block.
#[test]
fn interprets_casts_above_the_modulus() {
    let result =
        interpret_fixture("interp_casts_above_modulus").expect("interpretation should succeed");
    assert_eq!(result, Value::Unit, "main returns unit");
}

/// A false (non-const-folded) assertion interprets to `AssertionFailed`, not a clean pass.
#[test]
fn detects_false_assertion() {
    let project = NoirProject::new(negative_fixture("assert_fail")).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend compile");
    match interpret(&validated.program, validated.field_id) {
        Err(InterpretError::AssertionFailed { .. }) => {}
        other => panic!("expected AssertionFailed, got {other:?}"),
    }
}

#[test]
fn rejects_reachable_type_error() {
    let project = NoirProject::new(negative_fixture("reachable_error")).expect("project");
    assert!(compile_for_validation(&project, FieldId::linked()).is_err());
}

/// The `Prover.toml` input bridge: `interp_inputs_u64` with `x = 3` computes `x*2 + (p+1)` in u64.
#[test]
fn interprets_fixture_inputs_from_prover_toml() {
    let root = fixture("interp_inputs_u64");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");

    assert!(matches!(
        interpret_with_inputs(&validated.program, Vec::new(), validated.field_id),
        Err(InterpretError::InvalidInput(_))
    ));
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
    assert_eq!(
        result,
        Value::Int(IntValue {
            signed: true,
            bits: 64,
            value: BigInt::from(-1)
        })
    );
}

/// `u64` can exceed the Goldilocks modulus, so the compiler refuses `x as Field` there.
#[cfg(feature = "goldilocks")]
#[test]
fn goldilocks_rejects_u64_to_field_cast() {
    let project = NoirProject::new(negative_fixture("interp_cast_u64_to_field")).expect("project");
    let err = match compile_for_validation(&project, FieldId::linked()) {
        Ok(_) => panic!("u64 as Field must not compile under Goldilocks"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("cannot be cast to Field"), "{err}");
}

/// Under bn254 every `u64` is below the modulus and the cast is the identity on the value.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn bn254_casts_u64_to_field_exactly() {
    let project = NoirProject::new(negative_fixture("interp_cast_u64_to_field")).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = format!("x = \"{}\"", u64::MAX);
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
    let expected =
        acvm::FieldValue::try_from_biguint(u128::from(u64::MAX).into(), validated.field_id)
            .expect("u64::MAX is below the bn254 modulus");
    assert_eq!(result, Value::Field(expected));
}

/// `-2^32` has the pattern `p - 1`; `-1` has the pattern `2^64 - 1`, which exceeds the modulus.
#[cfg(feature = "goldilocks")]
#[test]
fn goldilocks_validates_i64_input_patterns() {
    let root = fixture("neg_interp_inputs_i64");
    let project = NoirProject::new(root).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let run = |toml: &str| {
        inputs_from_prover_toml(&validated.program, &validated.abi, toml, validated.field_id)
            .and_then(|inputs| {
                interpret_with_inputs(&validated.program, inputs, validated.field_id)
            })
    };
    let expected = |v: i64| {
        Value::Int(IntValue {
            signed: true,
            bits: 64,
            value: BigInt::from(v),
        })
    };
    for toml in ["x = -4294967296", "x = \"-4294967296\""] {
        assert_eq!(run(toml).expect(toml), expected(-4294967296), "{toml}");
    }
    assert_eq!(run("x = \"1\"").expect("x = \"1\""), expected(1));
    for toml in ["x = \"-1\"", "x = -1"] {
        assert!(
            matches!(run(toml), Err(InterpretError::InvalidInput(_))),
            "{toml}"
        );
    }
}

/// Struct inputs map by declaration order, not the alphabetical ABI map, at every nesting level.
/// `zeta*1000 + alpha*100 + (1+2+3) == 3706`.
#[test]
fn interprets_struct_input_by_declaration_order() {
    let root = fixture("interp_inputs_struct");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
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
        compile_for_validation(&project, FieldId::linked()).expect("frontend")
    };
    let run = |x: i32| {
        let input = Value::Int(IntValue {
            signed: true,
            bits: 32,
            value: BigInt::from(x),
        });
        interpret_with_inputs(&validated.program, vec![input], validated.field_id)
            .expect("interpret")
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

#[test]
fn renders_assert_message() {
    let project = NoirProject::new(negative_fixture("assert_fmt_msg")).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend compile");
    match interpret(&validated.program, validated.field_id) {
        Err(InterpretError::AssertionFailed {
            message: Some(m), ..
        }) => assert_eq!(
            m,
            "sum=45 field=0x03 array=[1, 2] tuple=(7,) point=Point { x: 4, y: true } choice=Choice::Some(9)"
        ),
        other => panic!("expected AssertionFailed with a rendered message, got {other:?}"),
    }
}

#[test]
fn stored_format_strings_reject_erased_type_names() {
    let validated = compile_source(
        "struct Pair { x: u32 } fn main(flag: bool) {
         let p = Pair { x: 7 }; let message = f\"value: {p}\"; assert(flag, message); }",
        FieldId::linked(),
    );
    let result = interpret_with_inputs(
        &validated.program,
        vec![Value::Bool(false)],
        validated.field_id,
    );
    assert!(
        matches!(result, Err(InterpretError::Unsupported(ref message)) if message.contains("erased type metadata")),
        "{result:?}"
    );
}

/// A struct in a format string loses its name in the mono AST; that is fine on the way to
/// `print`, whose text is dropped.
#[test]
fn printed_format_strings_may_interpolate_erased_aggregates() {
    let validated = compile_source(
        "struct Pair { x: u32 } fn main() { let p = Pair { x: 7 }; println(f\"value: {p}\"); }",
        FieldId::linked(),
    );
    assert_eq!(
        interpret(&validated.program, validated.field_id).unwrap(),
        Value::Unit
    );
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
    let validated = compile_for_validation(&project, FieldId::linked()).unwrap();
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap();

    let result = interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap();
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
    let validated = compile_for_validation(&project, FieldId::linked()).unwrap();
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();

    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap();
    let value = interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap();
    let expected = expected_return_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
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
    let validated = compile_for_validation(&project, FieldId::linked())
        .expect("goldilocks frontend should produce a mono-AST for a stdlib-free u64 program");

    // main(x: u64) -> u64 = x * 2 + (p + 1). With x = 3: 6 + 18446744069414584322.
    let x = Value::Int(IntValue {
        signed: false,
        bits: 64,
        value: BigInt::from(3u64),
    });
    let result =
        interpret_with_inputs(&validated.program, vec![x], validated.field_id).expect("interpret");
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

#[test]
fn interprets_wide_integers() {
    for bits in [34u32, 36, 66, 126, 128] {
        assert_fixture_return(
            &format!("interp_width_{bits}"),
            Value::Int(IntValue::canonical(false, bits, BigInt::from(16))),
        );
    }
}

#[test]
fn rejects_invalid_integer_widths() {
    for (name, needle) in [
        ("width_odd", "`u33` is not a supported integer type"),
        ("width_gap", "`u10` is not a supported integer type"),
        (
            "width_above_max",
            "`u65538` is not a supported integer type",
        ),
        ("width_unresolved", "Could not resolve 'N' in path"),
    ] {
        let project = NoirProject::new(negative_fixture(name)).expect("project");
        let error = match compile_for_validation(&project, FieldId::linked()) {
            Ok(_) => panic!("{name}: validation accepted an invalid width"),
            Err(error) => error,
        };
        assert!(
            error.summary().contains(needle),
            "{name}: {}",
            error.summary()
        );
    }
}

#[test]
fn interprets_stdlib_fixtures() {
    for (name, expected) in [
        (
            "interp_wrapping_ops",
            Value::Int(IntValue::canonical(false, 64, BigInt::from(7))),
        ),
        ("interp_hash_limbs", Value::Bool(false)),
        ("interp_field_lt", Value::Bool(true)),
        ("interp_derive_eq_hash", Value::Bool(false)),
    ] {
        assert_fixture_return(name, expected);
    }
}

#[cfg(not(feature = "goldilocks"))]
#[test]
fn bn254_accepts_input_above_the_goldilocks_modulus() {
    assert_fixture_return(
        "neg_wide_input_u66",
        Value::Int(IntValue::canonical(false, 66, BigInt::from(1u8) << 65usize)),
    );
}

#[cfg(feature = "goldilocks")]
#[test]
fn goldilocks_rejects_input_above_its_modulus() {
    let root = fixture("neg_wide_input_u66");
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project, FieldId::linked()).expect("frontend");
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    );
    assert!(
        matches!(inputs, Err(InterpretError::InvalidInput(_))),
        "{inputs:?}"
    );
}

// --- Differential oracle: interpreter vs Noir's own ACVM/Brillig executor (see `noir_oracle.rs`).
// Two independent lowerings (tree-walk vs full ACIR compile+execute) must agree on the return. ---

/// Run every argument-less stdlib test; bn254's crypto black boxes are the only coverage gap.
#[test]
fn the_stdlib_tests_pass_under_the_linked_field() {
    let root = temp_noir_package("test", "fn main() {}");
    let project = NoirProject::new(root.path().to_path_buf()).expect("project");
    let (mut passed, mut gaps, mut failures) = (0, Vec::new(), Vec::new());
    for test in stdlib_tests(&project, FieldId::linked()).expect("frontend") {
        let outcome = match test.program {
            Err(error) if matches!(test.scope, TestScope::ShouldFailWith { .. }) => {
                expected_test_outcome(&test.scope, Some(error.summary().to_string()))
            }
            Err(error) => Err(format!("monomorphization: {error}")),
            Ok(program) => {
                match panic::catch_unwind(AssertUnwindSafe(|| {
                    interpret(&program, FieldId::linked())
                })) {
                    Err(payload) => Err(format!("panic: {}", panic_message(payload.as_ref()))),
                    Ok(Err(InterpretError::Unsupported(what)))
                        if matches!(
                            what.as_str(),
                            "intrinsic 'multi_scalar_mul'"
                                | "intrinsic 'embedded_curve_add'"
                                | "intrinsic 'poseidon2_permutation'"
                                | "intrinsic 'derive_pedersen_generators'"
                        ) =>
                    {
                        gaps.push(format!("{}: {what}", test.name));
                        continue;
                    }
                    Ok(result) => stdlib_test_outcome(&test.scope, result),
                }
            }
        };
        match outcome {
            Ok(()) => passed += 1,
            Err(why) => failures.push(format!("{}: {why}", test.name)),
        }
    }
    println!(
        "stdlib tests under {}: {passed} passed, {} gaps\n{}",
        FieldId::linked(),
        gaps.len(),
        gaps.join("\n")
    );
    assert!(passed > 0, "no stdlib tests ran");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Whether a test's result is the one its scope asks for.
fn stdlib_test_outcome(
    scope: &TestScope,
    result: Result<Value, InterpretError>,
) -> Result<(), String> {
    let failure = match result {
        Ok(_) => None,
        Err(
            error @ (InterpretError::Type(_)
            | InterpretError::Internal(_)
            | InterpretError::Unsupported(_)
            | InterpretError::InvalidInput(_)),
        ) => {
            return Err(error.to_string());
        }
        Err(InterpretError::AssertionFailed {
            message: Some(message),
            ..
        }) => Some(message),
        Err(error) => Some(error.to_string()),
    };
    expected_test_outcome(scope, failure)
}

fn expected_test_outcome(scope: &TestScope, failure: Option<String>) -> Result<(), String> {
    match (scope, failure) {
        (TestScope::None, None) | (TestScope::OnlyFailWith { .. }, None) => Ok(()),
        (TestScope::None, Some(failure)) => Err(failure),
        (TestScope::ShouldFailWith { .. }, None) => Err("did not fail".to_string()),
        (TestScope::ShouldFailWith { reason: None }, Some(_)) => Ok(()),
        (
            TestScope::ShouldFailWith {
                reason: Some(reason),
            },
            Some(failure),
        )
        | (TestScope::OnlyFailWith { reason }, Some(failure)) => {
            if failure.to_lowercase().contains(&reason.to_lowercase()) {
                Ok(())
            } else {
                Err(format!("failed with {failure:?} rather than {reason:?}"))
            }
        }
    }
}

#[test]
fn stdlib_expected_failures_match_noir() {
    let should_fail = TestScope::ShouldFailWith { reason: None };
    let reason = TestScope::ShouldFailWith {
        reason: Some("EXPECTED".into()),
    };
    let only = TestScope::OnlyFailWith {
        reason: "EXPECTED".into(),
    };
    for (scope, failure, passes) in [
        (&TestScope::None, None, true),
        (&TestScope::None, Some("expected"), false),
        (&should_fail, None, false),
        (&should_fail, Some("anything"), true),
        (&reason, Some("expected failure"), true),
        (&reason, Some("different failure"), false),
        (&only, None, true),
        (&only, Some("expected failure"), true),
        (&only, Some("different failure"), false),
    ] {
        assert_eq!(
            expected_test_outcome(scope, failure.map(str::to_string)).is_ok(),
            passes
        );
    }
    for error in [
        InterpretError::Internal("expected".into()),
        InterpretError::Type("expected".into()),
        InterpretError::InvalidInput("expected".into()),
        InterpretError::Unsupported("intrinsic 'blake3'".into()),
    ] {
        assert!(stdlib_test_outcome(&should_fail, Err(error)).is_err());
    }
}

/// Run one program through both the interpreter and Noir's executor and classify the comparison.
/// The executor runs even when the interpreter rejects the program, so a *false rejection* (interp
/// errors on something nargo runs fine) is caught, not hidden. Buckets: `"agree"`,
/// `"FALSE-REJECTION: ..."`, `"MISMATCH: ..."`, `"oracle-wrong: ..."`, `"interp-unsupported: ..."`
/// (tolerated gap), `"interp-panic: ..."`, `"interp-internal: ..."` (both always failures),
/// `"oracle-errored"`, `"both-errored"`.
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
        let validated = compile_for_validation(&project, FieldId::linked())
            .map_err(|e| (compile_error_of(&e).kind, e.to_string()))?;
        let inputs = match &prover_src {
            Some(src) => {
                inputs_from_prover_toml(&validated.program, &validated.abi, src, validated.field_id)
                    .map_err(|e| (comparable_error_of(&e).kind, e.to_string()))?
            }
            None => Vec::new(),
        };
        let value = interpret_with_inputs(&validated.program, inputs, validated.field_id)
            .map_err(|e| (comparable_error_of(&e).kind, e.to_string()))?;
        Ok::<_, (FailureKind, String)>((validated, value))
    }));
    let interp = match interp {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(kd)) => Err(kd),
        Err(payload) => Err((FailureKind::Panic, panic_message(payload.as_ref()))),
    };

    match (interp, executor_ok) {
        (Err((FailureKind::Panic, detail)), _) => format!("interp-panic: {detail}"),
        (Err((FailureKind::Internal, detail)), _) => format!("interp-internal: {detail}"),
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
                super::expected_return_from_prover_toml(
                    &validated.program,
                    &validated.abi,
                    src,
                    validated.field_id,
                )
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

/// Compare the fixtures supported by the BN254 executor with the interpreter.
#[cfg(not(feature = "goldilocks"))]
#[test]
fn oracle_matches_interpreter_smoke() {
    // interp_inputs_mixed is left out: its shape trips Noir's ACIR flattening pass, so the executor
    // cannot judge it (the interpreter still runs it — see `interprets_mixed_inputs`).
    for name in [
        "interp_basic",
        "interp_inputs_u64",
        "interp_inputs_i32",
        "interp_inputs_i64",
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
        "interp_field_lt",
        "interp_hash_limbs",
        "interp_derive_eq_hash",
        "interp_wrapping_ops",
    ] {
        let result = oracle_compare(&fixture(name));
        assert_eq!(
            result, "agree",
            "interpreter/executor disagreement on {name}: {result}"
        );
    }
}

/// Differential survey: run the whole `execution_success` corpus through the interpreter and
/// Noir's executor and fail on mismatches, false rejections, panics or internal errors.
/// Tolerated `interp-unsupported`
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
        if [
            "MISMATCH",
            "FALSE-REJECTION",
            "interp-panic",
            "interp-internal",
        ]
        .iter()
        .any(|prefix| result.starts_with(prefix))
        {
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
        "{} interpreter/executor failure(s) found",
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
    use acvm::FieldId;
    use mavros_compiler::{driver::Driver, project::Project};

    /// The integration driver and the pure-Noir frontend should agree on a stdlib-free fixture.
    #[test]
    fn integration_driver_agrees_with_pure_noir() {
        let root = fixture("interp_inputs_u64");
        let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");

        // pure-Noir side
        let noir = NoirProject::new(root.clone()).expect("noir project");
        let validated =
            compile_for_validation(&noir, FieldId::linked()).expect("pure-noir frontend");
        let noir_inputs = inputs_from_prover_toml(
            &validated.program,
            &validated.abi,
            &toml,
            validated.field_id,
        )
        .expect("noir inputs");
        let noir_result =
            interpret_with_inputs(&validated.program, noir_inputs, validated.field_id)
                .expect("noir interpret");

        // Integration side.
        let project = Project::new(root.clone()).expect("oracle project");
        let mut driver = Driver::new(project, false);
        driver.run_noir_compiler().expect("oracle compile");
        let oracle_program = driver.monomorphized_program();
        // The driver compiles for the field it is linked against.
        let oracle_inputs =
            inputs_from_prover_toml(oracle_program, driver.abi(), &toml, FieldId::linked())
                .expect("oracle inputs");
        let oracle_result = interpret_with_inputs(oracle_program, oracle_inputs, FieldId::linked())
            .expect("oracle interpret");

        assert_eq!(
            noir_result, oracle_result,
            "integration AST must interpret identically to pure-Noir"
        );
    }

    /// Exercises the `PackageSource` impl used by the optional oracle.
    #[test]
    fn compile_for_validation_accepts_oracle_project() {
        let project = Project::new(fixture("interp_basic")).expect("oracle project");
        let validated =
            compile_for_validation(&project, FieldId::linked()).expect("validate oracle project");
        let result = interpret(&validated.program, validated.field_id).expect("interpret");
        assert_eq!(result, Value::Unit);
    }
}
