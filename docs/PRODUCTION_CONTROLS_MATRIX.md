# Production Controls Matrix

Status: required controls inventory only. This does not claim production
readiness.

| Control | Required evidence before production |
| --- | --- |
| Production evidence controls | Exact artifact binding, full gate replay, runtime safety proof, redaction proof, rollback proof, and explicit release decision review. |
| Secret manager / KMS / HSM | Key custody design review, no plaintext private keys in process logs, credential rotation drill, and break-glass access review. |
| Production config profile | Live submit and live cancel disabled by default, per-account enablement, per-market enablement, amount caps, and operator approval. |
| Deployment runbook | Artifact SHA-256 verification, migration evidence, config diff review, preflight gate summary, and named operator approval. |
| Rollback runbook | Kill switch drill, sign-only fallback, cancel-only fallback, read-only fallback, and database rollback or forward-fix boundary. |
| Incident drill | Remote unknown, cancel failure, SDK failure, PostgreSQL unavailable, geoblock, low resource, and runtime degraded scenarios. |
| Alerting and dashboard | Runtime worker health, reconcile backlog, remote unknown count, idempotency conflict rate, SDK error rate, and audit export failure. |
| SLO / error budget | Safety SLO separated from availability SLO; safety breach freezes live submit even when availability is healthy. |
| Audit export / retention policy | Redacted immutable export, retention duration, deletion policy, access review, and per-order trace id. |
| Account risk limits | Account whitelist, per-order cap, per-day cap, exposure cap, and operator approval threshold. |
| Market risk limits | Market whitelist, market-level exposure cap, geoblock enforcement, and stale market data blocking rule. |
| Dependency update policy | Pinned SDK/dependencies, compatibility report, sign-only regression evidence, and rollback plan. |
| SDK upstream breakage playbook | Freeze live submit, downgrade to sign-only/read-only, preserve evidence, and require compatibility review before unfreeze. |

Production promotion is blocked until this matrix is backed by current evidence
for the exact release artifact. Current source must remain non-production unless
a reviewed release decision explicitly says otherwise.
