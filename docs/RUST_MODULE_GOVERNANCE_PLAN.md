# Rust module governance plan

This plan is the v0.25 structure-governance entry point. It is intentionally
behavior-preserving and must not introduce live submit, live cancel, signing
material exposure, or production-readiness claims.

## Entry criteria

- Current validation-promotion evidence is present under `evidence/current/`.
- Shadow-readiness drills have passed or are explicitly skipped with reason.
- No required gate is failing.

## Split order

1. Pure DTOs, enums, and error types.
2. Repository traits before implementation modules.
3. PostgreSQL implementations after trait boundaries are stable.
4. SDK adapter config, signer boundary, transport, dry-run, and error mapping.
5. Service orchestration only after lower-level modules are stable.

## Current progress

- First behavior-preserving split batch moved sign-only lifecycle and standard
  construction into separate `pmx-service::sign_only` submodules.
- PostgreSQL admin audit and execution lifecycle persistence were separated
  under `pmx-store::postgres_audit` implementation modules.
- SDK adapter plan mapping now separates normalization and validation helpers
  under `pmx-official-sdk-adapter::mapping` without changing public exports.
- SDK liveness now keeps SDK error normalization behind a feature-gated
  `liveness::error_normalization` module while preserving the public function.
- PostgreSQL runtime worker persistence now separates heartbeat writes,
  observation writes, and status queries under `pmx-store::postgres_worker`.
- PostgreSQL repository tests now separate runtime-worker health/status and
  order-lifecycle coverage into focused `pmx-store::postgres_tests` modules.
- PostgreSQL sign-only lifecycle PG tests now live in a focused
  `pmx-store::postgres_tests::sign_only` module.
- PostgreSQL admin audit and submit idempotency PG tests now live in focused
  `pmx-store::postgres_tests` modules.
- The remaining PostgreSQL schema, receipt/reservation, execution lifecycle,
  and runtime-state PG tests now live in focused `pmx-store::postgres_tests`
  modules; the parent file only keeps shared helpers and module declarations.
- In-memory store tests now live in focused `pmx-store::memory_tests` modules
  for common helpers, admin/sign-only, runtime observations, runtime workers,
  and order lifecycle.
- Service flow and sign-only orchestration tests now live in focused
  `pmx-service::service_tests` modules while preserving the same assertions.
- Service runtime-worker basics, heartbeat lease/continuous tick coverage, and
  non-live order lifecycle tests now live in focused
  `pmx-service::service_tests` modules while preserving the same assertions.
- The `pmx-service::service_tests` parent file now only keeps shared helpers
  and module declarations; all service tests live in focused submodules.
- HTTP PostgreSQL API E2E tests now live in focused
  `pmx-api::tests::http_postgres_e2e` modules; the parent file only keeps
  shared request/seed helpers and module declarations.
- Official SDK adapter tests now live in focused
  `pmx-official-sdk-adapter::tests` modules for canary/config, sign-only,
  mapping, liveness/error redaction, and feature-gated smoke/typecheck paths.
  The parent file only keeps shared helpers and module declarations.
- Runtime model tests now live in focused `pmx-runtime::runtime_tests` modules
  for breakdown/loop behavior and capability evaluations; the parent file only
  keeps module declarations.
- HTTP fake/in-memory API E2E tests now live in focused
  `pmx-api::tests::http_and_fake_e2e` modules for auth/smoke, scaffolded
  lifecycle coverage, and negative startup/object-graph paths; the parent file
  only keeps shared helpers and module declarations.
- Core domain tests now live in focused `pmx-core::domain_tests` modules for
  intent normalization, lifecycle transitions, and divergence classification;
  the parent file only keeps shared helpers and module declarations.
- Gateway tests now live in focused `pmx-gateway::tests` modules for post/cancel
  flows, signer-provider boundaries, and read-only reconcile-reader behavior;
  the parent file only keeps shared helpers and module declarations.
- Service specialized runtime-worker tests now live in focused
  `pmx-service::service_tests::runtime_worker_specialized` modules for
  resource/reconcile, websocket/geoblock, and crash-recovery coverage.
- Runtime model tests now also separate focused `breakdown_loop` and
  `evaluations` submodules by capability group, worker-loop behavior,
  provider-fed loop behavior, lease/resource evaluation, reconcile/websocket/
  geoblock evaluation, and crash-recovery evaluation.
- In-memory store admin/sign-only tests now live in focused
  `pmx-store::memory_tests::admin_sign_only` modules for admin-audit behavior
  and sign-only lifecycle behavior.
- The HTTP fake scaffold E2E path now uses local helper functions to keep the
  single end-to-end assertion flow readable without changing route coverage or
  assertions.

## Per-step rules

- One small module move per commit.
- No semantic changes during a move.
- No OpenAPI or Hermes contract changes unless the public API actually changes.
- No new dependencies unless justified by the moved boundary.
- Keep old tests passing before moving to the next split.

## Required checks after each split

```bash
cargo fmt --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
python validation/check_live_submit_guard.py
python validation/check_sign_only_lifecycle.py
python validation/check_runtime_worker_models.py
```

From the integration repository, also run:

```bash
python scripts/validate_contracts.py
python polymarket-execution-engine/validation/check_docs_evidence_governance.py
```

## Stop conditions

- Any behavior changes without explicit design approval.
- Any redaction, runtime fail-closed, lifecycle idempotency, or live-submit guard regression.
- Any uncertainty about whether a move changes public API behavior.
