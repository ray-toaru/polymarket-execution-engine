# Sign-only Dry-run Plan

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: executed on 2026-05-15 with explicit runtime gates and real signer credentials. Evidence: `evidence/2026-05-15/v0.10/15-sign-only-dry-run.log`.

## Goal

Exercise the official SDK order builder and signing path without calling `post_order`, without creating a `PostedOrder`, and without mutating remote Polymarket state.

## Preconditions

```text
PMX_RUN_SIGN_ONLY_DRY_RUN=1
PMX_ALLOW_SIGN_ONLY_DRY_RUN=1
PMX_ALLOW_LIVE_SUBMIT must be unset or false
live-submit Cargo feature must be disabled
kill switch may remain closed
repository must not mark the order as posted
```

## Allowed path

```text
ExecutionPlanSummary / internal plan
-> SDK order builder
-> SDK sign(...)
-> local SignedOrderEnvelope reference only
-> audit event: SIGN_ONLY_DRY_RUN_COMPLETED
```

## Forbidden path

```text
post_order(...)
post_orders(...)
cancel_order(...)
remote order id persistence as posted
reservation consume as posted
```

## Acceptance criteria

```text
- signed payload never leaves execution process
- Python control plane never receives signed payload
- receipt contains only a local signed_order_ref
- posted=false
- any live-submit enablement aborts the dry-run
```

## Current v0.10 boundary

```text
- LIMIT path is wired through official SDK builder + sign()
- token_id must resolve to a real market because SDK fetches tick size / fee metadata before sign
- MARKET path remains mapping-only; posting and live submit remain disabled
- no post_order call exists in the sign-only code path
```
