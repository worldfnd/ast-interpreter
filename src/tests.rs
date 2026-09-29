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

fn assert_fixture_unit_under_both_fields(name: &str) {
    let project = NoirProject::new(fixture(name)).expect("project");
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_for_validation(&project, field)
            .unwrap_or_else(|error| panic!("{name}: {field}: frontend: {error}"));
        assert_eq!(
            interpret(&validated.program, validated.field_id).expect("interpret"),
            Value::Unit,
            "{name}: {field}"
        );
    }
}

/// The fixture returns `expected` under both fields, from its `Prover.toml` inputs.
fn assert_fixture_return(name: &str, expected: Value) {
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        assert_fixture_return_under(field, name, expected.clone());
    }
}

fn assert_fixture_return_under(field: FieldId, name: &str, expected: Value) {
    let (result, recorded) = run_fixture(field, name);
    assert_eq!(result, expected, "{name}: {field}");
    assert_eq!(recorded, Some(expected), "{name}: {field}: recorded return");
}

/// The fixture's return under `field` from its `Prover.toml` inputs, and the return it records.
fn run_fixture(field: FieldId, name: &str) -> (Value, Option<Value>) {
    let root = fixture(name);
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation(&project, field)
        .unwrap_or_else(|error| panic!("{name}: {field}: frontend: {error}"));
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap_or_else(|error| panic!("{name}: {field}: inputs: {error}"));
    let result = interpret_with_inputs(&validated.program, inputs, validated.field_id)
        .unwrap_or_else(|error| panic!("{name}: {field}: interpret: {error}"));
    let recorded = expected_return_from_prover_toml(
        &validated.program,
        &validated.abi,
        &toml,
        validated.field_id,
    )
    .unwrap_or_else(|error| panic!("{name}: {field}: recorded return: {error}"));
    (result, recorded)
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
fn a_scalar_whose_abi_type_disagrees_with_its_program_type_is_refused() {
    use noirc_abi::input_parser::InputValue;
    use noirc_abi::{AbiType, Sign};
    use noirc_frontend::monomorphization::ast::Type;
    use noirc_frontend::shared::Signedness;

    let u64_abi = AbiType::Integer {
        sign: Sign::Unsigned,
        width: 64,
    };
    let u8_abi = AbiType::Integer {
        sign: Sign::Unsigned,
        width: 8,
    };
    let input = InputValue::Field(5u8.into());
    for (abi_type, typ) in [
        (AbiType::Field, Type::Bool),
        (u64_abi.clone(), Type::Bool),
        (u64_abi, Type::Integer(Signedness::Unsigned, 8)),
        (u8_abi, Type::Integer(Signedness::Signed, 8)),
        (AbiType::Boolean, Type::Field),
    ] {
        let result = crate::input::value_from_input(&input, &abi_type, &typ, FieldId::Bn254);
        assert!(
            matches!(result, Err(InterpretError::Internal(_))),
            "{abi_type:?} as {typ:?}: {result:?}"
        );
    }
}

#[test]
fn toml_bridge_reads_inputs_in_the_programs_field() {
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_source("fn main(x: Field) -> pub Field { x }", field);
        for (toml, expected) in [
            ("x = -1\nreturn = -1", -FieldValue::one(field)),
            ("x = 1\nreturn = 1", FieldValue::one(field)),
        ] {
            let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, toml, field)
                .unwrap_or_else(|error| panic!("{field}: {toml}: {error}"));
            let recorded =
                expected_return_from_prover_toml(&validated.program, &validated.abi, toml, field)
                    .unwrap_or_else(|error| panic!("{field}: {toml}: {error}"));
            assert_eq!(
                inputs,
                vec![Value::Field(expected.clone())],
                "{field}: {toml}"
            );
            assert_eq!(recorded, Some(Value::Field(expected)), "{field}: {toml}");
        }
    }
}

#[test]
fn the_input_bridge_follows_the_abi_boundary_vectors() {
    use noirc_abi::conformance::boundary_vectors;
    use noirc_abi::input_parser::Format;
    use noirc_abi::{AbiParameter, AbiType, AbiVisibility, Sign};
    use noirc_frontend::monomorphization::ast::Type;
    use noirc_frontend::shared::Signedness;

    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let config = FieldConfig::new(field);
        for vector in boundary_vectors(config) {
            let abi = noirc_abi::Abi {
                parameters: vec![AbiParameter {
                    name: "x".to_string(),
                    typ: vector.typ.clone(),
                    visibility: AbiVisibility::Private,
                }],
                return_type: None,
                error_types: Default::default(),
            };
            let typ = match &vector.typ {
                AbiType::Field => Type::Field,
                AbiType::Boolean => Type::Bool,
                AbiType::Integer { sign, width } => {
                    let signedness = match sign {
                        Sign::Unsigned => Signedness::Unsigned,
                        Sign::Signed => Signedness::Signed,
                    };
                    Type::Integer(signedness, *width)
                }
                other => panic!("boundary vectors are scalars, got {other:?}"),
            };
            let parsed = Format::Toml.parse(&format!("x = {}", vector.spelling), &abi, config);
            match (parsed, &vector.element) {
                (Ok(inputs), Some(element)) => {
                    let value =
                        crate::input::value_from_input(&inputs["x"], &vector.typ, &typ, field)
                            .unwrap_or_else(|error| panic!("{field}: {vector:?}: {error}"));
                    let expected = match &typ {
                        Type::Field => Value::Field(
                            FieldValue::try_from_biguint(element.clone(), field)
                                .expect("an accepted element is below the modulus"),
                        ),
                        Type::Bool => Value::Bool(*element == 1u8.into()),
                        Type::Integer(signedness, bits) => Value::Int(IntValue::canonical(
                            signedness.is_signed(),
                            *bits,
                            BigInt::from(element.clone()),
                        )),
                        _ => unreachable!(),
                    };
                    assert_eq!(value, expected, "{field}: {vector:?}");
                }
                (Err(_), None) => {}
                (parsed, element) => {
                    panic!("{field}: {vector:?}: parsed {parsed:?}, expected {element:?}")
                }
            }
        }
    }
}

#[test]
fn rejects_inputs_from_another_field() {
    let (field, other) = (FieldId::Bn254, FieldId::Goldilocks);
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

    // Caller-built values must be checked even inside shapes the ABI cannot express.
    let validated = compile_source("fn main(x: Field) -> pub Field { x }", field);
    for hidden in [
        Value::Ref(cell(&bad), false),
        Value::Array(vec![Value::Ref(cell(&bad), false)]),
        Value::tuple(vec![Value::Ref(cell(&bad), true)]),
        Value::FmtStr {
            fragments: Vec::new(),
            captures: vec![Value::tuple(vec![bad.clone()])],
        },
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
    assert_fixture_unit_under_both_fields("interp_casts_above_modulus");
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
    let validated = compile_for_validation(&project, FieldId::Bn254).expect("frontend");
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

/// `Prover.toml` inputs give the same result under both fields: signed i32 arithmetic
/// (`a = -7, b = 2` → `-121`), struct fields bound in declaration order rather than the ABI's
/// alphabetical one (`3706`), an array with a signed branch (`295`) and an enum `match` binding a
/// variant's payload (`x = 3` → `12`).
#[test]
fn interprets_prover_toml_inputs() {
    let u32v = |v: u32| Value::Int(IntValue::canonical(false, 32, BigInt::from(v)));
    for (name, expected) in [
        (
            "interp_inputs_i32",
            Value::Int(IntValue::canonical(true, 32, BigInt::from(-121))),
        ),
        ("interp_inputs_struct", u32v(3706)),
        ("interp_inputs_mixed", u32v(295)),
        ("interp_match_enum", u32v(12)),
    ] {
        for field in [FieldId::Bn254, FieldId::Goldilocks] {
            assert_eq!(run_fixture(field, name).0, expected, "{name}: {field}");
        }
    }
}

/// `u64` can exceed the Goldilocks modulus, so the compiler refuses `x as Field` there.
#[test]
fn goldilocks_rejects_u64_to_field_cast() {
    let project = NoirProject::new(negative_fixture("interp_cast_u64_to_field")).expect("project");
    let err = match compile_for_validation(&project, FieldId::Goldilocks) {
        Ok(_) => panic!("u64 as Field must not compile under Goldilocks"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("cannot be cast to Field"), "{err}");
}

/// Under bn254 every `u64` is below the modulus and the cast is the identity on the value.
#[test]
fn bn254_casts_u64_to_field_exactly() {
    let project = NoirProject::new(negative_fixture("interp_cast_u64_to_field")).expect("project");
    let validated = compile_for_validation(&project, FieldId::Bn254).expect("frontend");
    let toml = format!("hi = \"{}\"\nlo = \"{}\"", u32::MAX, u32::MAX);
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

#[test]
fn goldilocks_refuses_an_entry_point_integer_that_can_reach_its_modulus() {
    for name in [
        "interp_inputs_i64",
        "interp_return_i64",
        "neg_wide_input_u66",
    ] {
        let project = NoirProject::new(fixture(name)).expect("project");
        assert!(
            compile_for_validation(&project, FieldId::Bn254).is_ok(),
            "{name}: bn254 takes this entry point"
        );
        let error = match compile_for_validation(&project, FieldId::Goldilocks) {
            Ok(_) => panic!("{name}: this entry point must not compile under Goldilocks"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("Invalid type found in the entry point to a program"),
            "{name}: {error}"
        );
    }
}

/// A `&mut` threaded through `main -> twice -> bump` mutates one shared cell: `100 + 5 + 5 == 110`.
#[test]
fn interprets_reference_call_chain() {
    assert_fixture_return(
        "interp_refs_call_chain",
        Value::Int(IntValue::canonical(false, 32, BigInt::from(110))),
    );
}

#[test]
fn interprets_field_projections_through_references() {
    assert_fixture_unit_under_both_fields("interp_refs_offset_receiver");
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
fn stored_format_strings_render_with_their_type_names() {
    let validated = compile_source(
        "struct Pair { x: u32 }
         struct Wrap { p: Pair, tag: bool }
         fn main(which: u32) {
             let p = Pair { x: 7 };
             let message = f\"value: {p}\";
             println(message);
             let messages = [f\"a: {p}\", f\"b: {p}\"];
             let w = Wrap { p, tag: true };
             let picked = if which == 3 { f\"x {w}\" } else { f\"y {w}\" };
             let tuple = (f\"t0 {p}\", 5);
             assert(which != 0, message);
             assert(which != 1, messages[1]);
             assert((which != 2) & (which != 3), picked);
             assert(which != 4, tuple.0);
             let s = \"plain\";
             assert(which != 5, f\"nested str {s}\");
             let holder = (p, 1);
             assert(which != 6, f\"held {holder}\");
             let mut changing = Pair { x: 7 };
             let snapshot = f\"saved {changing}\";
             changing.x = 9;
             assert(changing.x == 9);
             assert(which != 7, snapshot);
             let mut reassigned = f\"old {p}\";
             let q = changing;
             reassigned = f\"new {q}\";
             assert(which != 8, reassigned);
             let empty = f\"no captures\";
             assert(which != 9, empty);
             if which == 10 {
                 std::static_assert(false, f\"static {p}\");
             }
             if which == 11 {
                 let x: u32 = 7;
                 std::static_assert(false, f\"x={x}\");
             }
             if which == 12 {
                 std::static_assert(false, f\"no captures\");
             }
             if which == 13 {
                 std::static_assert(false, f\"outer {message}\");
             }
         }",
        FieldId::linked(),
    );
    for (which, expected) in [
        (0, "value: Pair { x: 7 }"),
        (1, "b: Pair { x: 7 }"),
        (2, "y Wrap { p: Pair { x: 7 }, tag: true }"),
        (3, "x Wrap { p: Pair { x: 7 }, tag: true }"),
        (4, "t0 Pair { x: 7 }"),
        (5, "nested str plain"),
        (6, "held (Pair { x: 7 }, 0x01)"),
        (7, "saved Pair { x: 7 }"),
        (8, "new Pair { x: 9 }"),
        (9, "no captures"),
        (10, "static Pair { x: 7 }"),
        (11, "x=7"),
        (12, "no captures"),
        (13, "outer value: Pair { x: 7 }"),
    ] {
        let input = Value::Int(IntValue::canonical(false, 32, BigInt::from(which)));
        let result = interpret_with_inputs(&validated.program, vec![input], validated.field_id);
        match result {
            Err(InterpretError::AssertionFailed {
                message: Some(message),
                ..
            }) => assert_eq!(message, expected, "which = {which}"),
            other => panic!("which = {which}: {other:?}"),
        }
    }
    let input = Value::Int(IntValue::canonical(false, 32, BigInt::from(14)));
    assert_eq!(
        interpret_with_inputs(&validated.program, vec![input], validated.field_id).unwrap(),
        Value::Unit
    );
}

/// A `main` with inputs interprets correctly from `Prover.toml`. `assert_statement` has `x == y == 3`.
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
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_for_validation(&project, field).unwrap();
        let inputs = inputs_from_prover_toml(
            &validated.program,
            &validated.abi,
            &toml,
            validated.field_id,
        )
        .unwrap();

        let result = interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap();
        assert_eq!(result, Value::Unit, "{field}");
    }
}

/// Differential correctness: the interpreter's computed return value matches the expected output
/// Noir's corpus records in `Prover.toml`. `arithmetic_binary_operations` returns the `Field` 10
/// under both fields, so this verifies the actual value, not merely that interpretation didn't
/// error.
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
    let toml = std::fs::read_to_string(root.join("Prover.toml")).unwrap();
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_for_validation(&project, field).unwrap();
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
            "{field}: interpreter output must match Noir's recorded return"
        );
    }
}

#[test]
fn interprets_integer_widths() {
    for (bits, returned) in [
        (2u32, 3),
        (10, 16),
        (33, 16),
        (34, 16),
        (36, 16),
        (66, 16),
        (126, 16),
        (128, 16),
        (16384, 16),
    ] {
        let returned_width = if bits > 63 { 8 } else { bits };
        assert_fixture_return(
            &format!("interp_width_{bits}"),
            Value::Int(IntValue::canonical(
                false,
                returned_width,
                BigInt::from(returned),
            )),
        );
    }
}

#[test]
fn rejects_invalid_integer_widths() {
    const RULE: &str = "integer widths are every width from 2 to 16384";
    for (name, messages, note) in [
        (
            "width_above_max",
            &["`u16385` is not a supported integer type"][..],
            RULE,
        ),
        (
            "width_zero",
            &["`u0` is not a supported integer type"][..],
            RULE,
        ),
        (
            "width_one",
            &[
                "`u1` is not a supported integer type",
                "`i1` is not a supported integer type",
            ][..],
            "`u1` has been removed, use `bool` instead",
        ),
        (
            "width_unresolved",
            &["Could not resolve 'N' in path"][..],
            "",
        ),
    ] {
        let project = NoirProject::new(negative_fixture(name)).expect("project");
        let error = match compile_for_validation(&project, FieldId::linked()) {
            Ok(_) => panic!("{name}: validation accepted an invalid width"),
            Err(error) => error,
        };
        for message in messages {
            assert!(
                error.summary().contains(message),
                "{name}: {}",
                error.summary()
            );
        }
        assert!(error.detail().contains(note), "{name}: {}", error.detail());
        assert!(
            !error.summary().contains("Could not resolve 'u"),
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
            Value::Int(IntValue::canonical(false, 32, BigInt::from(7))),
        ),
        ("interp_hash_limbs", Value::Bool(false)),
        ("interp_field_lt", Value::Bool(true)),
        ("interp_derive_eq_hash", Value::Bool(false)),
        (
            "interp_refcount_constrained",
            Value::Int(IntValue::canonical(false, 32, BigInt::from(0))),
        ),
        (
            "interp_str_bytes",
            Value::Array(
                [0x41u8, 0xFF, 0x42]
                    .into_iter()
                    .map(|byte| Value::Int(IntValue::canonical(false, 8, BigInt::from(byte))))
                    .collect(),
            ),
        ),
    ] {
        assert_fixture_return(name, expected);
    }
}

#[test]
fn bn254_accepts_input_above_the_goldilocks_modulus() {
    assert_fixture_return_under(
        FieldId::Bn254,
        "neg_wide_input_u66",
        Value::Int(IntValue::canonical(false, 66, BigInt::from(1u8) << 65usize)),
    );
}

// --- Differential oracle: interpreter vs Noir's own ACVM/Brillig executor (see `noir_oracle.rs`).
// Two independent lowerings (tree-walk vs full ACIR compile+execute) must agree on the return. ---

/// Run every argument-less stdlib test; only explicitly disabled crypto is a coverage gap.
#[test]
fn the_stdlib_tests_pass_under_bn254() {
    stdlib_tests_pass_under(FieldId::Bn254);
}

#[test]
fn the_stdlib_tests_pass_under_goldilocks() {
    stdlib_tests_pass_under(FieldId::Goldilocks);
}

fn stdlib_tests_pass_under(field: FieldId) {
    let root = temp_noir_package("test", "fn main() {}");
    let project = NoirProject::new(root.path().to_path_buf()).expect("project");
    let (mut passed, mut gaps, mut failures) = (0, Vec::new(), Vec::new());
    for test in stdlib_tests(&project, field).expect("frontend") {
        let outcome = match test.program {
            Err(error) if matches!(test.scope, TestScope::ShouldFailWith { .. }) => {
                expected_test_outcome(&test.scope, Some(error.summary().to_string()))
            }
            Err(error) => Err(format!("monomorphization: {error}")),
            Ok(program) => {
                match panic::catch_unwind(AssertUnwindSafe(|| interpret(&program, field))) {
                    Err(payload) => Err(format!("panic: {}", panic_message(payload.as_ref()))),
                    Ok(Err(InterpretError::Unsupported(what)))
                        if !cfg!(feature = "bn254-crypto")
                            && matches!(
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
        "stdlib tests under {field}: {passed} passed, {} gaps\n{}",
        gaps.len(),
        gaps.join("\n")
    );
    assert!(passed > 0, "{field}: no stdlib tests ran");
    assert!(failures.is_empty(), "{field}:\n{}", failures.join("\n"));
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
                        Some(r) => match crate::input::value_from_input(
                            &iv,
                            &r.abi_type,
                            ret_ty,
                            FieldId::linked(),
                        ) {
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
#[test]
fn oracle_matches_interpreter_smoke() {
    // interp_inputs_mixed is left out: its shape trips Noir's ACIR flattening pass, so the executor
    // cannot judge it (the interpreter still runs it — see `interprets_prover_toml_inputs`).
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
        "interp_refs_offset_receiver",
        "interp_refcount_constrained",
        "interp_str_bytes",
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
// `#[cfg(feature = "mavros-oracle")]` then.
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
