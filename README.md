# ast-interpreter

A small Rust interpreter for Noir's monomorphized AST (`noirc_frontend::monomorphization::ast`).

It runs over bn254 or Goldilocks and compares field-independent values such as integers, booleans,
arrays, tuples, and structs. Cross-field comparisons ignore differences in `Field` values.

## Using it

```toml
[dependencies]
ast-interpreter = { git = "https://github.com/worldfnd/ast-interpreter.git", rev = "<rev>" }
```

Use `interpret` for self-checking programs with no inputs, or `interpret_with_inputs` when `main` takes
arguments. For `Prover.toml` inputs, use `inputs_from_prover_toml` and
`expected_return_from_prover_toml`.

Crates that pass Noir AST values into this interpreter must use the same pinned `noirc_frontend` and
`acvm` sources, spelled the same way. Cargo keys a git dependency on the URL *and* the reference, so
`branch = "x"` and `rev = "<head of x>"` are different sources: you get two `FieldElement` types and no
error until the two halves meet. `acvm::FieldElement` is selected at compile time.

The `mavros-oracle` feature is a placeholder. Its `mavros-compiler` dependency is commented out, so
enabling the feature fails the build with a `compile_error!` saying so.

`InterpretError` separates bad caller data (`InvalidInput`), runtime range errors
(`ValueOutOfRange`), invalid AST value shapes (`Type`), and interpreter invariant failures
(`Internal`). It and `FailureKind` are `#[non_exhaustive]`.

## Building and testing

Rust 1.89.0 is pinned in `rust-toolchain.toml`. Run these commands from this directory so
`.cargo/config.toml` supplies the stack size Noir's frontend needs.

```sh
cargo build
make test            # Tests under bn254 and Goldilocks
```

## STATUS.md

`STATUS.md` records the pinned compiler's behavior on Noir's `execution_success` corpus and this
crate's fixtures: one row per program with the compile, run and recorded-return checks under each
field, the cross-field verdict, whether both monomorphized ASTs project to the same hash, and a
fingerprint of the underlying records. CI regenerates the file and fails on changes; intentional
changes need the updated rows and an explanation in the PR. The full JSON dumps are written to
`target/status/` and uploaded as CI artifacts.

```sh
make status          # Sweep both fields and render STATUS.md; about 7 minutes per sweep
make status-check    # Regenerate and compare with the committed STATUS.md
```

The sweep uses `../noir`, or the checkout specified by `NOIR_CHECKOUT`. Its revision must match
the compiler pin in `Cargo.toml`, and the corpus and its path dependencies must be clean.
Change the compiler pin and interpreter code in separate PRs unless a compiler API change requires both.

`tests::oracle_survey_execution_success` separately compares the interpreter with Noir's executor;
its doc comment has the command. Unsupported intrinsics, including `field_less_than`,
`array_refcount`, and `vector_refcount`, remain explicit coverage gaps.
