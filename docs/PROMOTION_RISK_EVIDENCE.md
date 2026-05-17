# Promotion risk evidence

This document indexes the current v0.23.1 validation-promotion evidence. It is an
evidence map, not a production-readiness claim.

## Current conclusion

Status: `shadow-ready candidate`

The current source and package evidence supports non-live shadow readiness only.
Live submit, live cancel, and production deployment remain blocked.

## Evidence sources

- Canonical manifest: `polymarket-execution-engine/evidence/current/manifest.json`
- Environment: `polymarket-execution-engine/evidence/current/environment.json`
- Logs: `polymarket-execution-engine/evidence/current/logs/`
- Release manifest: `polymarket-execution-engine/release/manifest.json`
- External artifact hash sidecars: `polymarket-dual-project-v0.23.0.zip.sha256` and `polymarket-dual-project-v0.23.0.zip.evidence.json`

## P1 risk closure

### Audit payload redaction E2E

Status: covered for the current non-live lifecycle/API paths.

Evidence:

- `11-sdk-adapter-test.log` and `12-sdk-adapter-typecheck.log` include adapter redaction tests.
- `05-http-fake-e2e.log` covers fake API lifecycle behavior.
- `15-http-postgres-e2e.log` covers PostgreSQL-backed lifecycle API behavior.
- `22-v0-23-lifecycle-api-guard.log` and `25-contract-validation.log` guard OpenAPI/Hermes/Rust parity for `RedactedPayloadEnvelope`.

Boundary:

- This does not prove a production logging pipeline or external observability backend.
- Raw private keys, CLOB secrets, raw signed payloads, raw signatures, and signed order envelopes must remain absent from public API responses, audit queries, lifecycle records, and release artifacts.

### Runtime degraded policy

Status: covered for current fail-closed policy.

Evidence:

- `04-cargo-test-workspace-non-api.log` includes runtime/policy tests for degraded worker behavior.
- `15-http-postgres-e2e.log` includes degraded snapshot and blocked decision behavior.
- `21-runtime-worker-model-guard.log` checks runtime worker model invariants.
- `25-contract-validation.log` checks cross-repository contract consistency.

Boundary:

- Current proof covers modeled runtime states. Real worker outage, market data outage, and reconciliation drift drills remain next-stage work.

### PostgreSQL sign-only lifecycle concurrency

Status: covered for current repository/API invariants.

Evidence:

- `13-pg-migration.log` applies the sign-only lifecycle schema, including `client_event_id` and partial unique index DDL.
- `14-pg-store-tests.log` covers PostgreSQL repository tests.
- `15-http-postgres-e2e.log` covers PostgreSQL-backed API lifecycle paths.
- `20-sign-only-lifecycle-guard.log` and `22-v0-23-lifecycle-api-guard.log` statically guard lifecycle/idempotency/API invariants.

Boundary:

- Current proof is enough for shadow-readiness, not live submit readiness.
- Repository/API evidence covers replay, idempotency, terminal-state, advisory
  lock, and partial unique index invariants. Live submit readiness still needs
  canary evidence before any funds-moving path.

## Shadow-readiness drill evidence

- Shadow execution would-submit drill: `29-shadow-execution-drill.log`.
- Reconciliation drift drill: `31-reconciliation-drift-drill.log`.
- Kill-switch and rollback drill: `32-kill-switch-rollback-drill.log`.
- Migration framework guard: `33-migration-framework-guard.log`.
- Runtime worker status query guard: `42-runtime-worker-status-query.log`.
- Observability evidence guard: `43-observability-evidence.log`.
- Automatic evidence manifest sections: `shadow_execution_validation`,
  `reconciliation_drift_validation`, `rollback_kill_switch_validation`,
  `runtime_worker_status_validation`, and
  `observability_evidence_validation`.

Boundary:

- The shadow drill performs a public market read and local candidate-order construction only.
- The reconciliation and rollback drills are local simulations. They are not a substitute for production runbooks or live remote reconciliation.
- Per-order lifecycle trace propagation now has a durable `order_events.correlation_id`
  field, order-event query API, shadow/reconcile trace IDs, and a
  manifest-bound observability guard; external dashboarding remains future work.
- Runtime worker status inspection now has a read-only `/v1/runtime/workers`
  API and manifest-bound guard; external dashboarding remains future work.
- The migration framework records version/checksum evidence for local validation. It does not yet prove production migration rollback, dry-run, or drift handling.
