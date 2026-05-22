# Cancel / Reconcile State-machine Next Work

> Status: current v0.26.0 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

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

Current v0.25 progress:

- `ExecutorService::record_non_live_cancel_request()` records
  `CancelRequested` into the local order lifecycle when the order already
  exists and belongs to the supplied `account_id`; missing orders return
  `NotFound`, and cross-account cancel attempts return `Conflict`.
- `ExecutorService::record_non_live_reconcile_observation()` records local
  `ReconcileOpen` / `ReconcileMissing` / `ReconcileUnknown` observations for
  existing orders.
- The admin cancel API now requires that local order-lifecycle write to
  succeed before returning a reconcile-required non-live receipt. Unknown
  orders are not reported as accepted cancels.
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
- Same `correlation_id` replay for cancel/reconcile order events is idempotent;
  reusing the same correlation id for a different event is rejected.
- `UNKNOWN` remote observations are persisted as `ReconcileUnknown` without
  advancing the local state, preserving an audit trail for operator review.
- Non-live cancel, reconcile, and divergence payloads are produced through
  typed service-layer payload constructors before serialization. The public
  JSON remains compatible, while the source shape is restricted to
  `kind`, optional `correlation_id`, `reason_len`, classification fields where
  needed, and `no_remote_side_effect`.

Boundary:

- Unknown local orders are not treated as confirmed cancelled.
- No live cancel or remote reconcile call is enabled.
