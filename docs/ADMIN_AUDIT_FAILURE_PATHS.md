# Admin audit failure paths

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.15 recorded accepted admin actions. v0.16 additionally records rejected admin requests when a principal has already been authorized and the request fails validation.

Covered examples:

- `CancelOrder` with empty `account_id`, `order_id`, or `reason`.
- `Reconcile` with empty reason.

Requests denied before a principal is known, such as missing or invalid bearer token, are not yet persisted. That requires a separate unauthenticated security-event sink and is intentionally not mixed with admin audit records.
