# Referee recipes. `just --list` shows them; CI runs the same recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

# The Noir checkout whose corpus the ledgers photograph. It must be checked out at the pinned
# compiler revision; point NOIR_CHECKOUT at a worktree of that revision when the sibling is not.
export NOIR_CHECKOUT := env_var_or_default("NOIR_CHECKOUT", justfile_directory() / ".." / "noir")

default:
    @just --list

# The fixture suite under both fields.
test:
    cargo test --locked
    cargo test --locked --features goldilocks

# Photograph one field into target/ledger/<field>.json and ledger/<field>.md (single-threaded, ~7 min).
sweep field="bn254":
    cargo test --locked --lib {{ if field == "goldilocks" { "--features goldilocks" } else { "" } }} ledger::dump_ledger -- --ignored --nocapture

# Compare the two field dumps and write ledger/cross-field.md.
compare:
    cargo test --locked --lib ledger::cross_field_diff -- --ignored --nocapture

# Regenerate all three ledgers.
ledger: (sweep "bn254") (sweep "goldilocks") compare

# Regenerate the ledgers and fail when they differ from the committed copies.
ledger-check: ledger
    git diff --exit-code --stat -- ledger/
