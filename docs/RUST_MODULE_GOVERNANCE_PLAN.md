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
- Service runtime-worker basic tests now also live in finer-grained
  `pmx-service::service_tests::runtime_worker_basic` modules for
  provider-backed state, runtime signal/tick fail-closed behavior, and
  runtime-worker status query coverage.
- Service runtime-worker lease tests now also live in finer-grained
  `pmx-service::service_tests::runtime_worker_lease` modules for
  continuous provider snapshots, fail-closed lease election, and
  persisted/in-PostgreSQL lease-owner parity coverage.
- Service non-live order-lifecycle tests now live in focused
  `pmx-service::service_tests::non_live_order_lifecycle` modules for
  cancel/reconcile recording and divergence escalation behavior.
- Service sign-only tests now also live in finer-grained
  `pmx-service::service_tests::sign_only` modules for lifecycle sequencing and
  standard sign-only construction/redaction behavior.
- Service specialized runtime-worker tests now also separate focused
  `resource_refresh` and `reconcile_backlog` modules instead of sharing a
  mixed resource/reconcile parent test file.
- Runtime model tests now also separate focused `breakdown_loop` and
  `evaluations` submodules by capability group, worker-loop behavior,
  provider-fed loop behavior, lease/resource evaluation, reconcile/websocket/
  geoblock evaluation, and crash-recovery evaluation.
- Core lifecycle domain types and transitions now live in focused
  `pmx-core::domain::lifecycle` modules for sign-only lifecycle,
  order-lifecycle transitions, and divergence/reconcile classification.
- Core plan/control-plane models now live in focused `pmx-core::domain::plan`
  modules for decision results, execution summaries/submit receipts, redaction
  envelopes, and control-plane request/receipt models.
- Core base domain primitives now live in focused `pmx-core::domain::base`
  modules for shared errors, typed ids, decimal validation, and canonical JSON
  hashing/serialization helpers.
- In-memory store admin/sign-only tests now live in focused
  `pmx-store::memory_tests::admin_sign_only` modules for admin-audit behavior
  and sign-only lifecycle behavior.
- In-memory store sign-only tests now also separate focused happy-path,
  idempotency, and reject-path modules.
- Runtime breakdown capability tests now also separate focused blocking,
  capability-group, and store-write fail-closed modules.
- PostgreSQL order-lifecycle tests now also separate focused persistence,
  replay, and reconcile-backlog modules.
- PostgreSQL sign-only tests now also separate focused persistence and
  concurrent idempotency modules.
- PostgreSQL runtime-state tests now also separate focused state-loading/
  degradation and observation-write modules.
- Service standard sign-only implementation now separates request validation,
  digest/ref derivation, and lifecycle persistence/replay helpers.
- Service heartbeat lease tick implementation now separates lease-election
  recording and store-backed heartbeat/status persistence while preserving the
  same public tick helpers and fail-closed semantics.
- PostgreSQL order-lifecycle write implementation now separates upsert,
  replay lookup/conflict handling, and event-apply SQL paths.
- In-memory order-lifecycle store implementation now separates write,
  event-query, and reconcile-backlog helpers while preserving the same trait
  surface.
- In-memory lifecycle store implementation now separates execution-lifecycle
  and sign-only lifecycle helpers while preserving the same trait surface.
- In-memory execution store implementation now separates normalized-intent/
  snapshot, decision, plan-summary, and reservation/receipt helpers while
  preserving the same trait surface.
- Service binding helpers now separate hash-input DTOs, sign-only lifecycle
  append validation, and snapshot/decision binding verification while
  preserving the same exports.
- Runtime helper logic now separates freshness horizon checks, worker-status
  aggregation, and observation-application helpers while preserving the same
  helper exports.
- PostgreSQL runtime-state loading now separates account/collateral lookup,
  worker-heartbeat row collection, and runtime-worker observation loading while
  preserving the same `RuntimeStateStore` implementation.
- API backend lifecycle delegation now separates execution-event,
  order-lifecycle, and sign-only/receipt helpers while preserving the same
  backend method surface.
- API route assembly now separates bootstrap/router construction and health
  endpoint helpers while preserving the same exported app builders.
- PostgreSQL support helpers now separate database-error normalization,
  JSON-payload loading, and runtime-state enum/status conversion helpers while
  preserving the same helper exports.
- PostgreSQL migration helpers now separate manifest/checksum, apply flow, and
  applied-migration recording while preserving the same `apply_schema` entry.
- API admin reconcile support now separates shared auth/correlation context,
  placeholder reconcile validation, and local reconcile validation while
  preserving the same route behavior.
- API in-memory/PostgreSQL E2E tests now serialize process-env token mutation
  so local `cargo test -p pmx-api` remains deterministic under parallel test
  scheduling.
- API read routes now separate submit-receipt reads, lifecycle-event queries,
  and runtime-worker status queries while preserving the same route surface.
- API flow routes now separate intent/snapshot/decision, plan compile/submit,
  and sign-only lifecycle handlers while preserving the same route surface.
- In-memory order-lifecycle tests now also separate focused cancel-requested,
  replay/conflict, invalid-transition, and reconcile-backlog modules.
- The HTTP fake scaffold E2E path now uses local helper functions to keep the
  single end-to-end assertion flow readable without changing route coverage or
  assertions.
- The HTTP fake scaffold E2E path now also keeps compile, submit/sign-only,
  admin, and public-query phases in focused helper modules while preserving the
  same single test flow.
- The HTTP PostgreSQL smoke E2E path now keeps compile/submit, sign-only,
  admin lifecycle, and public-query phases in focused helper modules while
  preserving the same single test flow.
- The HTTP PostgreSQL runtime E2E path now keeps runtime-state/degraded checks
  and ready-plan/blocked-submit verification in focused helper modules while
  preserving the same single test flow.

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
