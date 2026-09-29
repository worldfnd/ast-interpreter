# ast-interpreter

A small Rust interpreter for Noir's monomorphized AST (`noirc_frontend::monomorphization::ast`).

It runs over bn254 or Goldilocks and compares field-independent values such as integers, booleans,
arrays, tuples, and structs. Cross-field comparisons ignore differences in `Field` values.

A `Field` value carries the field it belongs to, and the interpreter takes its field from the label
the monomorphized program was compiled under, so one build runs a program under any supported
field. The ABI input parser reads values in that field too; it holds them in the linked
`acvm::FieldElement`, so a bn254 build parses bn254 and Goldilocks inputs alike. Only the ACVM
executor oracle computes in the linked field, bn254.

## Using it

```toml
[dependencies]
ast-interpreter = { git = "https://github.com/worldfnd/ast-interpreter.git", rev = "<rev>" }
```

Use `interpret` for self-checking programs with no inputs, or `interpret_with_inputs` when `main` takes
arguments. Both take the `FieldId` the program was compiled under, which
`MonomorphizationOutput::field_id` records. For `Prover.toml` inputs, use `inputs_from_prover_toml`
and `expected_return_from_prover_toml`, which read the values in that field. Inputs built directly
as `Value`s must follow the same rules: every `FieldValue` must belong to the program's field, and
every `IntValue` must be a value of the width and signedness it declares.

Crates that pass Noir AST values into this interpreter must use the same pinned `noirc_frontend` and
`acvm` sources, spelled the same way. Cargo keys a git dependency on the URL *and* the reference, so
`branch = "x"` and `rev = "<head of x>"` are different sources: you get two `FieldElement` types and no
error until the two halves meet. `acvm::FieldElement` is selected at compile time.

The `bn254-crypto` feature, on by default, routes the embedded-curve, Pedersen-generator and
Poseidon2 built-ins to `bn254_blackbox_solver`; they take bn254 values only, which is the one field
where Noir's standard library reaches them.

The `mavros-oracle` feature builds the differential oracle against Mavros (`src/mavros_oracle.rs`,
whose module doc has the sweep command). It needs a `../mavros` checkout pinned to the same Noir
revision, and LLVM 22 for the Mavros build. Its `mavros-compiler` dependency is commented out in
`Cargo.toml`, so CI needs no Mavros checkout; the module doc says what to un-comment to run it.

`InterpretError` separates bad caller data (`InvalidInput`), runtime range errors
(`ValueOutOfRange`), invalid AST value shapes (`Type`), and interpreter invariant failures
(`Internal`). It and `FailureKind` are `#[non_exhaustive]`.

## Building and testing

Rust 1.89.0 is pinned in `rust-toolchain.toml`. Run these commands from this directory so
`.cargo/config.toml` supplies the stack size Noir's frontend needs.

```sh
cargo build
make test            # Both feature builds test bn254 and Goldilocks programs
```

## STATUS.md

`STATUS.md` records what the pinned compiler and this interpreter do on Noir's `execution_success`
corpus and this crate's fixtures: one row per program with the compile, run and recorded-return
checks under each field, the cross-field verdict, whether both monomorphized ASTs project to the
same hash, and a fingerprint of the underlying records. CI regenerates the file and fails on
changes; intentional changes need the updated rows and an explanation in the PR. The full JSON
dumps are written to `target/status/` and uploaded as CI artifacts.

```sh
make status          # Sweep both fields and render STATUS.md; about 7 minutes per sweep
make status-check    # Regenerate and compare with the committed STATUS.md
```

The sweep uses `../noir`, or the checkout specified by `NOIR_CHECKOUT`. Its revision must match
the compiler pin in `Cargo.toml`, and the corpus and its path dependencies must be clean.
Change the compiler pin and interpreter code in separate PRs unless a compiler API change
requires both.

`tests::the_stdlib_tests_pass_under_bn254` and `tests::the_stdlib_tests_pass_under_goldilocks` run
every argument-less `#[test]` of the standard library through the interpreter under their field;
one it cannot run fails them, except bn254's own crypto when `bn254-crypto` is off.
`tests::oracle_survey_execution_success` separately compares the interpreter with Noir's executor;
its doc comment has the command. Unsupported intrinsics, such as a reference count taken in
unconstrained code, remain explicit coverage gaps.
