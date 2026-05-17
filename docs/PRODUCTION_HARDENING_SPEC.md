# Production Hardening Spec

Status: engineering specification only. This is not production readiness
evidence.

## Secret Custody

- Use reviewed secret manager, KMS, or HSM-backed signing.
- Private keys, CLOB secrets, raw signed payloads, raw signatures, and signed
  order envelopes must never be logged, exported through API, or persisted in
  audit/query tables.
- Rotation drill must include revoke, replace, deploy, verify, and rollback.

## Deployment And Rollback

- Deployment preflight must verify artifact SHA-256, evidence manifest SHA-256,
  migration status, config diff, live gates, and operator approval.
- Rollback must support config kill switch, downgrade to sign-only, cancel-only
  fallback, read-only fallback, and database forward-fix boundary.
- A migration rollback that can lose order/audit truth is forbidden; use
  forward-fix unless formally reviewed.

## Observability

- Required dashboard signals: runtime worker health, reconcile backlog, remote
  unknown count, idempotency conflicts, SDK error categories, audit export
  failures, and per-order trace id.
- Required alerts: live gate unexpectedly enabled, remote unknown freeze,
  geoblock blocked/error, PostgreSQL unavailable, SDK remote unknown spike,
  stale worker heartbeat, and audit export failure.

## SLO And Error Budget

- Safety SLO is separate from availability SLO.
- Any safety SLO breach freezes live submit before availability goals are
  considered.
- Error budget burn must not auto-enable live submit after recovery.

## Audit Export And Retention

- Audit export must be immutable, redacted, trace-id-linked, and access-reviewed.
- Retention duration, deletion policy, and legal hold behavior must be defined
  before production.
- Export failures must alert and block production promotion.

## Risk Limits

- Account whitelist, Market whitelist, per-order cap, per-day cap, exposure cap,
  and operator approval threshold are required before live canary.
- Remote unknown freeze overrides all limits and blocks further submit attempts.

## Dependency And SDK Breakage

- SDK and critical dependencies remain pinned.
- Every SDK update requires compatibility report, sign-only regression evidence,
  rollback plan, and upstream breakage playbook review.
