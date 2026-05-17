# Validation Evidence (Rust + PostgreSQL Milestone)

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Date: 2026-05-14

Execution root: `/workspaces/universal/rust/polymarket_dual_project`
Evidence root: `/tmp/pmx_validation`

## 1) Rust compile/test gates

- `cargo check --workspace` -> pass
- `cargo fmt --check` -> pass
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` -> pass
- `cargo test --workspace` -> pass

Evidence files:
- `01-cargo-check-final.log`
- `02-cargo-fmt-check.log`
- `03-cargo-clippy.log`
- `04-cargo-test.log`

## 2) HTTP auth + Fake E2E

- Added integration test: `crates/pmx-api/tests/http_and_fake_e2e.rs`
- Result: `05-http-auth-fake-e2e.log` (1 test passed)
- Verified scenarios:
  - Missing Authorization -> `401`
  - Invalid/malformed token -> `401/403`
  - Service token has service scope only
  - Admin scope required for kill-switch path

## 3) PostgreSQL concurrency proof

- PostgreSQL environment started in container
- Local schema/table created:
  - DB: `pmx`
  - User: `pmx`
  - Table: `execution_idempotency`
- Proof: advisory lock contention on `pg_advisory_lock(557)` shows second session blocking until first session releases lock.

Evidence files:
- `06-pg-concurrency-A.log`
- `06-pg-concurrency-B.log`
- `06-pg-concurrency-proof.log`
- Derived block duration observed: ~15664 ms

## 4) Repository state touched for this milestone

- HTTP/fake E2E test added under `crates/pmx-api/tests/`
- test/dev dependency in `crates/pmx-api/Cargo.toml` (`tower = { version="0.5", features=["util"] }`)
- Multiple scaffold/quality-fix edits in core/gateway/authz/policy/runtime/store crates from clippy/fmt adjustments and test plumbing
