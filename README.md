# ast-interpreter

A small Rust interpreter for Noir's monomorphized AST (`noirc_frontend::monomorphization::ast`).

It can run the same AST over bn254 or Goldilocks and compare field-independent values such as integers,
booleans, arrays, tuples, and structs. The default build uses Noir only. The optional `mavros-oracle`
feature is kept for integration checks.

## Using it

```toml
[dependencies]
ast-interpreter = { git = "https://github.com/worldfnd/ast-interpreter.git" }
```

Use `interpret` for self-checking programs with no inputs, or `interpret_with_inputs` when `main` takes
arguments. For `Prover.toml` inputs, use `inputs_from_prover_toml` and
`expected_return_from_prover_toml`.

Crates that pass Noir AST values into this interpreter must use the same pinned `noirc_frontend` and
`acvm` sources. `acvm::FieldElement` is selected at compile time, so mismatched Noir revisions can create
incompatible field types.

Main exports: `interpret`, `interpret_with_inputs`, `Value`, `IntValue`, `DiffValue`,
`DiffOutcome`, `values_equivalent`, `outcomes_equivalent`, and `InterpretError`.

## Building and testing

```sh
cargo build
```

```sh
cargo +nightly-2026-04-22 test
cargo +nightly-2026-04-22 test --features goldilocks
```

Some survey tests read Noir's upstream corpus from a sibling `../noir` checkout and self-skip when it is
absent.
