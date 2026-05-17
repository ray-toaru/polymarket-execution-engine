# Testing Strategy

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Current tests available here

- Python static contract validation from repo root.
- Python control tests.

## Rust tests required next

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## HTTP tests required after compile

- missing token -> 401/403
- invalid token -> 403
- service token can call service paths
- service token cannot call admin paths
- admin token can call admin paths
- admin token does not silently fallback to service token
- 202 status for submit/cancel/reconcile/kill-switch

## PG tests required before real adapter

- migration smoke
- idempotency replay
- idempotency conflict on request fingerprint mismatch
- concurrent submit does not double reserve
- reservation lifecycle binds to order id before active use
