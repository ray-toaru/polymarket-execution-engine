# AGENTS.md — Rust crates common rules

## Scope

Applies to all crates under `polymarket-execution-engine/crates/`.

## Common rules

- Treat crate-local `AGENTS.md` files as refinements; do not weaken repository or execution-engine safety boundaries.
- Preserve Rust workspace settings: edition `2024`, Rust `1.88`, locked dependency resolution.
- Keep public types stable unless the OpenAPI, Hermes client, tests, and docs are updated in the same change.
- Prefer small behavior-preserving refactors. Do not combine large module moves with semantic changes.
- Fail closed for authorization, runtime health, payload redaction, lifecycle terminal states, and live side-effect gates.
- Add or update unit tests for behavior changes. When Rust tooling is unavailable, mark Rust validation as missing evidence rather than passed.

## Expected checks

```bash
cargo fmt --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```
