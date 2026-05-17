# Productionization runbook

Status: v0.27 governance scaffold. This is not a production-readiness claim.

## Required controls before production

- Production evidence controls: apply `PRODUCTION_EVIDENCE_CONTROLS.md` before
  any production promotion decision.
- Secret manager: private keys, CLOB credentials, API tokens, and database
  credentials must move to a reviewed secret manager, KMS, or HSM-backed flow.
- Production config profile: live submit and live cancel must remain disabled by
  default and enabled only per account, market, amount, and strategy.
- Deployment runbook: include preflight gates, artifact hash verification,
  schema migration evidence, config diff review, and operator approval.
- Rollback runbook: include config-level kill switch, downgrade to sign-only,
  cancel-only fallback, read-only fallback, and database migration recovery
  limits.
- Incident drill: include remote unknown, cancel failure, SDK failure,
  PostgreSQL unavailable, geoblock, low resource, and degraded runtime workers.
- Alerting and dashboard: include per-order trace id, runtime worker health,
  reconcile backlog, remote unknown count, idempotency conflicts, SDK errors,
  and audit export failures.
- SLO and error budget: define availability and safety metrics separately; a
  safety breach must override availability goals.
- Audit export and retention policy: define immutable export destination,
  redaction guarantees, retention duration, deletion policy, and access review.
- Account and market risk limits: define whitelist, per-order cap, per-day cap,
  exposure cap, and operator approval threshold.
- Dependency update policy: pin SDK and critical dependencies, require
  compatibility report, rollback plan, and sign-only regression evidence.
- SDK upstream breakage playbook: freeze live submit, fall back to sign-only or
  read-only, preserve evidence, and require compatibility review before
  unfreezing.

## Production-ready decision rule

`production-ready is forbidden` unless all of the following have direct evidence:

- current full gates pass for the exact artifact hash;
- credentialed non-trading smoke passes;
- sign-only dry-run passes without raw signed payload exposure;
- canary submit/cancel evidence exists and is explicitly reviewed;
- rollback, kill-switch, cancel-only fallback, and incident drills pass;
- secret manager, monitoring, retention, deployment, and rollback runbooks are
  reviewed and linked from the release decision.

Until then, the maximum claim is a non-production candidate status such as
`validated source candidate`, `shadow-ready candidate`, or `canary-ready
candidate`.
