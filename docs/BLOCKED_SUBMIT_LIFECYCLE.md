# Blocked submit lifecycle event

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.17 adds a local execution lifecycle event around blocked submit attempts.

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
receipt_id
```

## Non-live guarantee

The event is written before/around the blocked receipt path only. It does not imply signing, posting, cancellation, or any Polymarket remote mutation.

## Required tests

- `postgres_records_execution_lifecycle_event`
- `http_postgres_runtime_rows_can_reach_ready_plan_but_submit_still_blocks`
