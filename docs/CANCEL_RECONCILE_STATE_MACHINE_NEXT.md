# Cancel / Reconcile State-machine Next Work

> Status: current v0.24.0 shadow-ready baseline documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.22 keeps live cancel disabled and adds clearer reconcile classification in `pmx-core`.

Current core helper:

```text
RemoteUnknown -> QueryRemoteOpenOrder
PartialRemoteUnknown -> ConfirmMissingOrEscalate
Failed -> OperatorRequired
Other states -> Noop
```

Next non-live work:

- Add fake-gateway cancel lifecycle tests.
- Persist cancel lifecycle events in PostgreSQL.
- Model `not_canceled` as non-terminal unless reconcile confirms remote truth.
- Add stale `RemoteUnknown` escalation into operator-required reconcile.
- Ensure cancel/reconcile never claim terminal state based only on request submission.

Current v0.24 progress:

- `ExecutorService::record_non_live_cancel_request()` records
  `CancelRequested` into the local order lifecycle when the order already
  exists.
- `ExecutorService::record_non_live_reconcile_observation()` records local
  `ReconcileOpen` / `ReconcileMissing` observations for existing orders.
- The admin cancel API now attempts the local order-lifecycle write while still
  returning a reconcile-required non-live receipt and preserving the execution
  lifecycle audit event.
- `pmx-core::classify_order_lifecycle_divergence()` classifies local-vs-remote
  divergence for `OPEN`, `MISSING`, and `UNKNOWN` remote observations.
- `ExecutorService::reconcile_order_lifecycle_divergence()` applies that
  classification to existing local orders and persists only local
  `order_events`; it does not call any remote cancel or submit endpoint.
- The public `/v1/admin/reconcile` request can optionally carry `order_id` and
  `remote_observation` together; when provided, it records the same local
  order-lifecycle divergence and still performs no remote side effect.
- Repeated `MISSING` observations escalate `RemoteUnknown ->
  PartialRemoteUnknown -> Failed` so operator-required paths are explicit.

Boundary:

- Unknown local orders are not treated as confirmed cancelled.
- No live cancel or remote reconcile call is enabled.
