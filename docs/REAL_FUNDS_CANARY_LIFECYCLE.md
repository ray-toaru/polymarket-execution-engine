# REAL_FUNDS_CANARY_LIFECYCLE

This document defines the local lifecycle closure for the real-funds canary path. It does not
authorize live submit or live cancel.

## State Model

- `PREFLIGHT_READY`: preflight inputs, approval hash, artifact hash, evidence manifest hash, market, and caps are recorded locally.
- `READY_BUT_LIVE_DISABLED`: the run is structurally ready, but release policy still blocks live submit.
- `REMOTE_UNKNOWN_FREEZE`: any remote-unknown outcome freezes further submit attempts and requires escalation.
- `OPERATOR_REQUIRED`: an operator must decide the recovery path before any further canary action.
- `SIMULATED_RECONCILED`: the local drill reconciled the run without remote side effects.

## Store Guarantees

- `idempotency replay`: the same `(account_id, idempotency_key)` and same request fingerprint returns the existing run.
- `idempotency conflict`: reusing the same idempotency key with a different request is rejected.
- `runtime truth binding`: an armed real-funds canary must prove durable
  kill-switch, live-submit gate, idempotency lease, and order/cancel
  reconciliation state before the SDK path may post.
- `remote_side_effects = false`
- `raw_signed_order_exposed = false`
- `simulated reconcile` records only local lifecycle state.

The lifecycle store must not expose private keys, CLOB secrets, raw signed payloads, raw signatures,
or signed order envelopes. It records hashes, refs, lifecycle state, and non-sensitive metadata only.
