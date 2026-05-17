# State Machine Notes

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Order Lifecycle

```text
PLANNED
  -> SIGNED
  -> POST_REQUESTED
  -> POSTED
  -> PARTIALLY_FILLED
  -> FILLED
```

Cancellation path:

```text
POSTED/PARTIALLY_FILLED
  -> CANCEL_REQUESTED
  -> CANCEL_REMOTE_ACCEPTED
  -> CANCEL_CONFIRMED
```

Uncertainty path:

```text
POST_REQUESTED or CANCEL_REQUESTED
  -> REMOTE_UNKNOWN
  -> RECONCILE_OPEN or RECONCILE_MISSING
```

Important rule: cancel request success is not confirmed cancellation.

## Reservation Lifecycle

```text
PENDING -> ACTIVE -> CONSUMED
PENDING/ACTIVE -> RELEASED
PENDING/ACTIVE -> ORPHANED
```

Each order must have its own reservation truth. Bundle-level success cannot be the only accounting event.

## Submit Idempotency

The intended identity is:

```text
account_id + execution_id + idempotency_key

submit_attempt is executor-internal and must not be supplied by the control plane. Same identity + same request fingerprint while PROCEEDING returns InProgress/RetryLater, not Proceed.
```

A retry with the same idempotency key should replay the stored response if terminal. It must not create a second remote side effect.
