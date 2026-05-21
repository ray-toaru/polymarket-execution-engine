# Admin audit persistence

> Status: current v0.25.0 shadow-ready SDK sign-only baseline documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.15 introduces `AdminAuditStore` and records accepted admin operations through the execution service.

Covered operations:

- kill switch updates,
- cancel-order scaffold requests,
- reconcile scaffold requests.

The audit event records:

- principal subject,
- operation name,
- request fingerprint where available,
- correlation id where available,
- result string,
- database timestamp in PostgreSQL.

Query behavior:

- `limit` is bounded to `1..=500`.
- `before_audit_id` is a stable older-page cursor.
- returned pages are oldest-to-newest within the selected page.
- operation, principal, result, and correlation-id filters are applied before
  cursor pagination.

Current boundary: this is not yet a complete compliance audit subsystem. It
does not persist unauthorized requests without a principal, and cancel/reconcile
remain non-live scaffold operations. `/v1/admin/cancel-order` records local
cancel intent and returns a reconcile-required receipt without calling a remote
cancel API. `/v1/admin/reconcile` records local reconcile context and optional
local order-lifecycle divergence without remote reconciliation reads.
