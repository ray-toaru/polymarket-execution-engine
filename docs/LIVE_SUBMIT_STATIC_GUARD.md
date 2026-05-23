# Live submit static guard

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Purpose

The project is still pre-live. The official SDK adapter may contain explicit safety gates for future `live-submit`. The service layer may contain only the explicit `submit_plan_with_gateway` pipeline for fake-gateway and future wiring tests; API/bootstrap production paths still default to fail-closed and do not wire a live gateway.

`ExecutorService::submit_plan` rejects `SubmitMode::Live` with `LIVE submit mode is fail-closed until gateway posting is wired through the executor service`. The only service-layer code path allowed to reach a gateway is `submit_plan_with_gateway`, which is intentionally explicit and remains outside API/bootstrap wiring until a future release decision changes that boundary.

## v0.19 guard

`validation/check_live_submit_guard.py` checks:

- the official SDK adapter source has no `.post_order(` or `.post_orders(` call after comments are stripped;
- the public OpenAPI contract does not expose signed/live-submit terms such as `SignedOrderEnvelope`, `signed_payload`, `private_key`, `clob_secret`, or `post_order`.
- any future live-submit canary must satisfy `LiveCanaryPreconditions`,
  including compile feature, env gate, config gate, kill switch, runtime worker,
  geoblock, repository reservation, idempotency key, reconcile worker,
  account/market whitelist, size cap, daily cap, operator approval, and
  cancel-only fallback.
- `default_blocked_live_canary_preconditions()` keeps every future live canary
  integration point blocked until all gates are explicitly populated.
- `pmx-service` remote post call sites are limited to `submit/live.rs`; that path
  must use explicit `submit_plan_with_gateway`, pre-sign and pre-post runtime
  checks, and redacted lifecycle payloads.
- submit idempotency is lease/owner protected in both memory and PostgreSQL
  stores: a fresh `PROCEEDING` row returns `InProgress`; an expired lease can
  create a new `Proceed` attempt with a new owner token; finish must match the
  current owner token and `PROCEEDING` status before recording a response.

The fake gateway crate is intentionally outside the static guard because its in-memory `post_order` is a deterministic test double, not a Polymarket remote side effect.

## Limitations

This is a static guard, not a proof of absence for all future dynamic paths. It must be combined with Rust tests, OpenAPI validation, release review, and explicit runtime gates before any live adapter work.

## Required next step

The guard is wired into:

```bash
polymarket-execution-engine/validation/run_current_gates.sh
```

The expected log is:

```text
18-live-submit-static-guard.log
```
