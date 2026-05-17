# Admin audit persistence

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.15 introduces `AdminAuditStore` and records accepted admin operations through the execution service.

Covered operations:

- kill switch updates,
- cancel-order scaffold requests,
- reconcile scaffold requests.

The audit event records:

- principal subject,
- operation name,
- request fingerprint where available,
- result string,
- database timestamp in PostgreSQL.

Current boundary: this is not yet a complete compliance audit subsystem. It does not persist unauthorized requests without a principal, and cancel/reconcile still remain scaffold operations.
