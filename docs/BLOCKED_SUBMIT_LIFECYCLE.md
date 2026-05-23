# Blocked submit lifecycle event

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

The current blocked submit path records a local execution lifecycle event without
persisting an order reservation and without any remote side effect.

Submit requests must declare `mode`. The only mode that reaches this path is
`BLOCKED_DRY_RUN`; `LIVE` fails closed before idempotency ownership is opened.

## Purpose

Before any remote side-effect path is introduced, submit attempts should already leave an auditable local trace. This prevents future work from adding sign/post behavior without first passing through a lifecycle recording boundary.

## Current event

```text
SUBMIT_BLOCKED_BEFORE_REMOTE
```

Payload includes:

```text
submit_attempt
plan_status
no_remote_side_effect=true
reservation_written=false
receipt_id
```

## Non-live guarantee

The event is written before/around the blocked receipt path only. It does not imply signing, posting, cancellation, or any Polymarket remote mutation.

## Required tests

- `postgres_records_execution_lifecycle_event`
- `http_postgres_runtime_rows_can_reach_ready_plan_but_submit_still_blocks`
- `live_submit_mode_fails_closed_until_gateway_is_wired`
