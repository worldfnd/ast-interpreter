# ast-interpreter

A small Rust interpreter for Noir's monomorphized AST (`noirc_frontend::monomorphization::ast`).

It runs over bn254 or Goldilocks and compares field-independent values such as integers, booleans,
arrays, tuples, and structs. Cross-field comparisons ignore differences in `Field` values;
per-field ledgers record their exact values.

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

## The ledgers

The three files in `ledger/` record the pinned compiler's behavior on Noir's `execution_success`
corpus and this crate's fixtures. Per-field rows include each run step, errors, returned values,
and a hash of the monomorphized AST. `cross-field.md` records agreements, expected gaps, and
divergences. Known compiler failures remain visible in the baseline.

CI regenerates the ledgers and fails on changes. Intentional changes need updated rows and an
explanation in the PR. Full JSON dumps are saved to `target/ledger/` and uploaded as CI artifacts.

```sh
make ledger          # Sweep both fields and compare; about 7 minutes per sweep
make ledger-check    # Regenerate and check against the committed ledgers
```

The sweep uses `../noir`, or the checkout specified by `NOIR_CHECKOUT`. Its revision must match
the compiler pin in `Cargo.toml`, and the corpus and its path dependencies must be clean.
Change the compiler pin and interpreter code in separate PRs unless a compiler API change requires both.

`tests::oracle_survey_execution_success` separately compares the interpreter with Noir's executor;
its doc comment has the command. Unsupported intrinsics, including `field_less_than`,
`array_refcount`, and `vector_refcount`, remain explicit coverage gaps.
