# Live Canary Preflight

This drill emits structured P1 canary-prep evidence only. It is a local
preflight proof and must run with no live submit and no live cancel capability
enabled.

Machine-readable checks:

- account_whitelisted
- market_whitelisted
- size_cap_ok
- daily_cap_ok
- operator_approved
- cancel_only_fallback_ready
- remote_unknown_freeze_clear
- reservation_ready
- idempotency_ready
- reconcile_ready

Negative scenarios must fail closed:

- missing operator approval;
- per-order cap exceeded;
- per-day cap exceeded;
- account not whitelisted;
- market not whitelisted;
- cancel-only fallback missing;
- remote unknown freeze active.

Expected output:

```text
preflight_status = local_ready_but_live_blocked
posted = false
cancelled = false
remote_side_effects = false
```

Passing this drill does not approve a live canary. It proves only that the
future canary preflight can be represented as current evidence and that common
negative scenarios remain fail-closed.

Current implementation boundary:

- `ExecutorService::submit_plan` still rejects `SubmitMode::Live` by default.
- `ExecutorService::submit_plan_with_gateway` is the explicit service-layer
  gateway pipeline for fake-gateway tests and future reviewed live wiring.
- `pmx-official-sdk-adapter` provides an explicit official SDK gateway bridge
  under the `live-submit` feature; raw signed SDK orders remain in process-local
  memory behind a digest reference and are not written to logs, receipts, or
  public API models.
- The explicit gateway path performs runtime checks before signing and again
  before remote post, records remote-unknown as operator-required evidence, and
  does not expose raw signed payloads.
- Cancel-only fallback is represented by an explicit service-layer gateway path
  for remote-posted orders. Remote-unknown cancel outcomes remain
  operator-required and do not auto-reopen live submit.
- Production API/bootstrap does not wire a live gateway in this release state.
