use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::corpus::{
    compile_error_of, copy_dir, corpus_dir, list_programs, noir_checkout, panic_message,
    temp_noir_package,
};
use super::diff::{FailureKind, comparable_error_of};
use super::expected_return_from_prover_toml;
use super::loader::NoirProject;
use super::validation_frontend::{
    Validated, compile_for_validation, compile_for_validation_with, stdlib_tests,
};
use super::{
    IntValue, InterpretError, Value, inputs_from_bytes, inputs_from_prover_toml, interpret,
    interpret_with_inputs,
};
use acvm::{FieldId, FieldValue};
use noirc_frontend::token::TestScope;
use num_bigint::{BigInt, BigUint};

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
    run_fixture_with(field, name, false)
}

/// [`run_fixture`] with Noir's benchmark mode chosen.
fn run_fixture_with(field: FieldId, name: &str, generic_builtins: bool) -> (Value, Option<Value>) {
    let root = fixture(name);
    let project = NoirProject::new(root.clone()).expect("project");
    let validated = compile_for_validation_with(&project, field, generic_builtins)
        .unwrap_or_else(|error| panic!("{name}: {field}: frontend: {error}"));
    let toml = std::fs::read_to_string(root.join("Prover.toml")).expect("Prover.toml");
    let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml)
        .unwrap_or_else(|error| panic!("{name}: {field}: inputs: {error}"));
    let result = interpret_with_inputs(&validated.program, inputs, validated.field_id)
        .unwrap_or_else(|error| panic!("{name}: {field}: interpret: {error}"));
    let recorded = expected_return_from_prover_toml(&validated.program, &validated.abi, &toml)
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
    }
}

/// Both bridges, the byte one for inputs and the field-element one for the executor's return,
/// refuse a scalar whose ABI type disagrees with its program type as an internal error.
#[test]
fn a_scalar_whose_abi_type_disagrees_with_its_program_type_is_refused() {
    use noirc_abi::input_parser::InputValue;
    use noirc_abi::{AbiType, Sign, bytes};
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
    let decoded = bytes::Value::Int(BigInt::from(5));
    for (abi_type, typ) in [
        (AbiType::Field, Type::Bool),
        (u64_abi.clone(), Type::Bool),
        (u64_abi, Type::Integer(Signedness::Unsigned, 8)),
        (u8_abi, Type::Integer(Signedness::Signed, 8)),
        (AbiType::Boolean, Type::Field),
    ] {
        let result = super::noir_oracle::value_from_input(&input, &abi_type, &typ, FieldId::Bn254);
        assert!(
            matches!(result, Err(InterpretError::Internal(_))),
            "{abi_type:?} as {typ:?}: {result:?}"
        );
        let result = crate::input::bind(&decoded, &abi_type, &typ);
        assert!(
            matches!(result, Err(InterpretError::Internal(_))),
            "bytes: {abi_type:?} as {typ:?}: {result:?}"
        );
    }
}

/// A `Prover.toml` and the blob it encodes to give the same values under every field: a negative
/// `i8`, and a `u128` above the Goldilocks modulus.
#[test]
fn toml_bridge_reads_the_same_values_under_every_field() {
    let source = "fn main(x: i8, y: u128) -> pub u128 { if x < 0 { y } else { 0 } }";
    let toml = "x = -1\ny = \"36893488147419103232\"\nreturn = \"36893488147419103232\"";
    // `x` in one byte, then `y` in sixteen little-endian bytes.
    let blob = [vec![0xff], vec![0; 8], vec![0x02], vec![0; 7]].concat();
    let x = Value::Int(IntValue::canonical(true, 8, BigInt::from(-1)));
    let y = Value::Int(IntValue::canonical(
        false,
        128,
        BigInt::from(1u8) << 65usize,
    ));
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_source(source, field);
        let (program, abi) = (&validated.program, &validated.abi);
        let inputs = inputs_from_prover_toml(program, abi, toml)
            .unwrap_or_else(|error| panic!("{field}: {error}"));
        assert_eq!(inputs, vec![x.clone(), y.clone()], "{field}");
        assert_eq!(
            inputs_from_bytes(program, abi, &blob).unwrap(),
            inputs,
            "{field}"
        );
        let recorded = expected_return_from_prover_toml(program, abi, toml).unwrap();
        assert_eq!(recorded, Some(y.clone()), "{field}");
        let returned = interpret_with_inputs(program, inputs, validated.field_id).unwrap();
        assert_eq!(returned, y, "{field}");
    }
}

/// The monomorphized type of an entry-point value of `typ`, as the frontend lowers it.
fn mono_type(typ: &noirc_abi::AbiType) -> noirc_frontend::monomorphization::ast::Type {
    use noirc_abi::{AbiType, Sign};
    use noirc_frontend::monomorphization::ast::Type;
    use noirc_frontend::shared::Signedness;

    match typ {
        AbiType::Field => Type::Field,
        AbiType::Boolean => Type::Bool,
        AbiType::Integer { sign, width } => {
            let signedness = match sign {
                Sign::Unsigned => Signedness::Unsigned,
                Sign::Signed => Signedness::Signed,
            };
            Type::Integer(signedness, *width)
        }
        AbiType::String { length } => Type::String(*length),
        AbiType::Array { length, typ } => Type::Array(*length, Rc::new(mono_type(typ))),
        AbiType::Tuple { fields } => Type::Tuple(fields.iter().map(mono_type).collect()),
        AbiType::Struct { fields, .. } => {
            Type::Tuple(fields.iter().map(|(_, typ)| mono_type(typ)).collect())
        }
    }
}

/// The input bridge reads every case of the byte codec's conformance vectors: an accepted case's
/// TOML and blob bind to the same values, which hold under every field, and a refused document or
/// blob is invalid input.
#[test]
fn the_input_bridge_follows_the_byte_vectors() {
    use crate::input::{bind_params, parse_prover_toml};
    use noirc_abi::bytes;
    use noirc_abi::conformance::byte_vectors;

    for vector in byte_vectors() {
        let types: Vec<_> = vector
            .abi
            .parameters
            .iter()
            .map(|p| mono_type(&p.typ))
            .collect();
        let parameters: Vec<(&str, &_)> = vector
            .abi
            .parameters
            .iter()
            .zip(&types)
            .map(|(p, typ)| (p.name.as_str(), typ))
            .collect();
        let from_toml = vector.toml.as_ref().map(|toml| {
            parse_prover_toml(toml, &vector.abi).and_then(|inputs| {
                let blob = bytes::encode_params(&vector.abi, &inputs.params)
                    .expect("parsed inputs encode");
                bind_params(&vector.abi, &parameters, &blob)
            })
        });
        let from_bytes = vector
            .bytes
            .as_ref()
            .map(|blob| bind_params(&vector.abi, &parameters, blob));
        for result in from_toml.iter().chain(&from_bytes) {
            match result {
                Ok(values) if vector.accept => {
                    for field in [FieldId::Bn254, FieldId::Goldilocks] {
                        crate::input::validate_inputs(values, field)
                            .unwrap_or_else(|e| panic!("{}: {field}: {e}", vector.name));
                    }
                }
                Err(InterpretError::InvalidInput(_)) if !vector.accept => {}
                result => panic!("{}: accept = {}: {result:?}", vector.name, vector.accept),
            }
        }
        if let (Some(Ok(toml)), Some(Ok(blob))) = (&from_toml, &from_bytes) {
            assert_eq!(toml, blob, "{}", vector.name);
        }
    }
}

/// A `Field` built by a caller must belong to the program's field. No entry point takes a
/// `Field`, so a stray one from another field is refused before the program runs, wherever it
/// hides.
#[test]
fn rejects_inputs_from_another_field() {
    let (field, other) = (FieldId::Bn254, FieldId::Goldilocks);
    let bad = Value::Field(FieldValue::one(other));
    let cell = |value: &Value| Rc::new(RefCell::new(value.clone()));
    let validated = compile_source("fn main(x: u8) -> pub u8 { x }", field);
    let one = Value::Int(IntValue::canonical(false, 8, BigInt::from(1)));
    assert_eq!(
        interpret_with_inputs(&validated.program, vec![one.clone()], field).unwrap(),
        one
    );
    for hidden in [
        bad.clone(),
        Value::Ref(cell(&bad), false),
        Value::Array(vec![Value::Ref(cell(&bad), false)]),
        Value::tuple(vec![Value::Array(vec![bad.clone()]), one.clone()]),
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
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");

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
    let inputs =
        inputs_from_prover_toml(&validated.program, &validated.abi, &toml).expect("inputs");
    let result =
        interpret_with_inputs(&validated.program, inputs, validated.field_id).expect("interpret");
    // The `Field` is `u64::MAX`, returned as its eight little-endian bytes.
    let byte = Value::Int(IntValue::canonical(false, 8, BigInt::from(0xffu8)));
    assert_eq!(result, Value::Array(vec![byte; 8]));
}

/// Integers whose values reach the Goldilocks modulus are entry-point types under every field,
/// since inputs and returns are read as bytes: a `u128` input of `2^65`, and `i64` inputs and
/// returns.
#[test]
fn wide_entry_point_integers_run_under_every_field() {
    for (name, expected) in [
        (
            "interp_inputs_u128",
            IntValue::canonical(false, 128, BigInt::from(1u8) << 65usize),
        ),
        (
            "interp_inputs_i64",
            IntValue::canonical(true, 64, BigInt::from(-4294967296i64)),
        ),
        (
            "interp_return_i64",
            IntValue::canonical(true, 64, BigInt::from(-1)),
        ),
    ] {
        assert_fixture_return(name, Value::Int(expected));
    }
}

/// A tuple or struct pattern parameter takes the whole argument, though the monomorphizer gives
/// the function one parameter per leaf of the pattern: `sum` gets `1 + 2`, `swap` returns `(1, 3)`
/// and the one-field `first` reads `5`, so `main` returns `9`.
#[test]
fn pattern_parameters_take_the_whole_argument() {
    let source = "
        struct Pair { a: u8, b: u8 }
        struct One { inner: (u8, u8) }
        fn sum(Pair { a, b }: Pair) -> u8 { a + b }
        fn swap((x, y): (u8, u8)) -> (u8, u8) { (y, x) }
        fn first(One { inner }: One) -> u8 { inner.0 }
        fn main(x: u8) -> pub u8 {
            let (s, t) = swap((sum(Pair { a: x, b: 2 }), 1));
            s + t + first(One { inner: (5, 6) })
        }
    ";
    let one = Value::Int(IntValue::canonical(false, 8, BigInt::from(1)));
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_source(source, field);
        let result = interpret_with_inputs(&validated.program, vec![one.clone()], field)
            .unwrap_or_else(|error| panic!("{field}: {error}"));
        assert_eq!(
            result,
            Value::Int(IntValue::canonical(false, 8, BigInt::from(9))),
            "{field}"
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
        let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml).unwrap();

        let result = interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap();
        assert_eq!(result, Value::Unit, "{field}");
    }
}

/// Differential correctness: the interpreter's computed return value matches the expected output
/// Noir's corpus records in `Prover.toml`. `arithmetic_binary_operations` returns the `u8` 10
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
        let inputs = inputs_from_prover_toml(&validated.program, &validated.abi, &toml).unwrap();
        let value = interpret_with_inputs(&validated.program, inputs, validated.field_id).unwrap();
        let expected = expected_return_from_prover_toml(&validated.program, &validated.abi, &toml)
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
        // Every width fixture takes `u8` inputs and returns a `u8`; the width under test lives in
        // a helper.
        assert_fixture_return(
            &format!("interp_width_{bits}"),
            Value::Int(IntValue::canonical(false, 8, BigInt::from(returned))),
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

/// Noir's benchmark mode compiles the field-generic twins of `Field::lt` and the wrapping ops on
/// bn254; they compute the same values as the bn254 halves, which `interprets_stdlib_fixtures`
/// pins.
#[test]
fn generic_builtins_keep_the_values_of_lt_and_wrapping_ops_on_bn254() {
    for (name, expected) in [
        ("interp_field_lt", Value::Bool(true)),
        (
            "interp_wrapping_ops",
            Value::Int(IntValue::canonical(false, 32, BigInt::from(7))),
        ),
    ] {
        let (result, _) = run_fixture_with(FieldId::Bn254, name, true);
        assert_eq!(result, expected, "{name}");
    }
}

/// The benchmark mode really selects the generic half: `Hash for u64` writes one element under
/// bn254 and two 32-bit limbs in its field-generic twin, which the recorder tells apart. The
/// mode acts on bn254 only, so Goldilocks hashes the limbs with or without it.
#[test]
fn generic_builtins_select_the_limb_hash_on_bn254() {
    // `Recorder::write` in the fixture folds each write as `state * MULTIPLIER + input`.
    const MULTIPLIER: u64 = 0x1_0000_0001_b3;
    // `main` hashes `x + 2^32` with `Prover.toml`'s `x = 7`.
    let wide: u64 = 7 + (1 << 32);
    // `main` returns the hash as eight little-endian bytes.
    let hash_under = |field: FieldId, generic_builtins: bool| {
        let (value, recorded) = run_fixture_with(field, "interp_generic_twins", generic_builtins);
        let Value::Array(bytes) = &value else {
            panic!("{value:?}")
        };
        let element = bytes
            .iter()
            .rev()
            .fold(BigUint::ZERO, |hash, byte| match byte {
                Value::Int(byte) => (hash << 8u8) + byte.value.to_biguint().expect("a byte"),
                other => panic!("{other:?}"),
            });
        (element, recorded == Some(value))
    };
    let (one_write, recorded_agrees) = hash_under(FieldId::Bn254, false);
    assert_eq!(one_write, BigUint::from(wide));
    assert!(recorded_agrees, "Prover.toml records bn254's one write");
    // The low limb first, then the high limb.
    let limbs = BigUint::from((wide & 0xffff_ffff) * MULTIPLIER + (wide >> 32));
    assert_eq!(hash_under(FieldId::Bn254, true).0, limbs);
    assert_eq!(hash_under(FieldId::Goldilocks, false).0, limbs);
    assert_eq!(hash_under(FieldId::Goldilocks, true).0, limbs);
}

/// Direct, mutual and lambda recursion stop at the interpreter's host-stack limit.
#[test]
fn unbounded_recursion_is_a_coverage_gap() {
    for name in [
        "simple_infinite_recursive_function",
        "simple_infinite_recursive_lambda",
        "mutually_recursive_simple_functions",
        "brillig_entry_points_shared_recursive",
    ] {
        let root = noir_checkout()
            .join("test_programs/execution_failure")
            .join(name);
        let project = NoirProject::new(root).expect("project");
        let validated = compile_for_validation(&project, FieldId::Bn254)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let error = interpret(&validated.program, validated.field_id).expect_err(name);
        assert!(
            matches!(&error, InterpretError::Unsupported(m) if m == "call depth exceeds 256"),
            "{name}: {error}"
        );
    }
}

#[test]
fn finite_recursion_at_the_call_limit_is_a_coverage_gap() {
    let root = temp_noir_package(
        "finite_recursion",
        "unconstrained fn recurse(n: u32) -> u32 {
            if n == 0 { 0 } else { recurse(n - 1) + 1 }
        }
        fn main(n: u32) -> pub u32 { unsafe { recurse(n) } }",
    );
    let project = NoirProject::new(root.path().to_path_buf()).expect("project");
    for field in [FieldId::Bn254, FieldId::Goldilocks] {
        let validated = compile_for_validation(&project, field).expect("frontend");
        let input = |n| Value::Int(IntValue::canonical(false, 32, BigInt::from(n)));
        // `main` and the terminating `recurse(0)` each consume one frame.
        let below = super::MAX_CALL_DEPTH as u32 - 2;
        assert_eq!(
            interpret_with_inputs(&validated.program, vec![input(below)], field).unwrap(),
            input(below)
        );
        assert!(matches!(
            interpret_with_inputs(&validated.program, vec![input(below + 1)], field),
            Err(InterpretError::Unsupported(_))
        ));
    }
    assert_eq!(
        super::noir_oracle::noir_execute_return(&project, Some("n = \"255\""))
            .expect("Noir executes recursion beyond the interpreter's limit"),
        Some(noirc_abi::input_parser::InputValue::Field(
            acvm::FieldElement::from(255u128)
        ))
    );
}

/// An entry point takes an integer of any width from 2 to 16384 bits, under every field: `u24`
/// is no width a circuit backend lowers natively.
#[test]
fn the_entry_point_takes_any_integer_width() {
    assert_fixture_return(
        "interp_entry_width_u24",
        Value::Int(IntValue::canonical(false, 24, BigInt::from(8))),
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
            Some(src) => inputs_from_prover_toml(&validated.program, &validated.abi, src)
                .map_err(|e| (comparable_error_of(&e).kind, e.to_string()))?,
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
                        Some(r) => match super::noir_oracle::value_from_input(
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

/// Compare the fixtures supported by the BN254 executor with the interpreter.
#[test]
fn oracle_matches_interpreter_smoke() {
    // interp_inputs_mixed is left out: its shape trips Noir's ACIR flattening pass, so the executor
    // cannot judge it (the interpreter still runs it — see `interprets_prover_toml_inputs`).
    for name in [
        "interp_basic",
        "interp_generic_twins",
        "interp_inputs_u64",
        "interp_inputs_i32",
        "interp_inputs_i64",
        "interp_inputs_struct",
        "interp_refs_struct_field",
        "interp_refs_call_chain",
        "interp_refs_nested_field",
        "interp_refs_double_deref_alias",
        "interp_refs_offset_receiver",
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
