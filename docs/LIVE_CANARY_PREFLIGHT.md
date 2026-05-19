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
