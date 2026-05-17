# PostgreSQL Repository Concurrency Proof

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Confirmed by supplied evidence

The supplied v0.4 evidence logs show the following repository tests passed:

```text
same_request_replay_is_persisted
fingerprint_mismatch_is_conflict
reservation_double_spend_is_prevented_concurrently
remote_unknown_is_persisted_conservatively
```

The logs are preserved in:

```text
../evidence/2026-05-14/
../../validation/2026-05-14-v0.4-repository-proof/
```

## What this proves

The evidence supports these claims for the tested repository implementation:

```text
- Same idempotency identity with same request fingerprint can replay stored response.
- Same idempotency identity with different request fingerprint is treated as conflict.
- Concurrent reservation reentry for the same execution/resource does not create duplicate active rows.
- REMOTE_UNKNOWN submit receipt is persisted conservatively rather than discarded.
```

## What it does not prove

```text
- Account-level budget aggregation across executions.
- Multi-leg saga recovery.
- Live CLOB side effects.
- WebSocket-driven reconcile correctness.
- Production readiness.
```

## Current implementation notes

- Advisory locks serialize transaction sections for idempotency attempt generation and reservation writes.
- SQL uniqueness remains the correctness backstop.
- `submit_attempt` is executor-generated; the control plane does not supply it.
- Canonical idempotency identity is `(account_id, execution_id, idempotency_key)`.

## Next tests

```text
- cross-execution budget exhaustion
- partial multi-leg remote unknown
- replay after process restart
- reservation release/consume race
- HTTP 409 on fingerprint mismatch through API layer
```
