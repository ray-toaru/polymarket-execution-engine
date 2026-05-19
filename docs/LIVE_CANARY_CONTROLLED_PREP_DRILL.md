# Live Canary Controlled Prep Drill

This drill is P2 evidence for controlled live canary preparation. It is
local-only and does not submit or cancel live orders.

Required canary gates:

- compile_feature_live_submit
- env_allow_live_submit
- config_allow_live_submit
- operator_approval_present
- account_whitelisted
- market_whitelisted
- tiny_size_cap
- limit_order_only
- idempotency_key_written
- repository_reservation_exists
- reconcile_after_submit_required
- remote_unknown_freezes_submit
- cancel_only_fallback_ready

Required behavior:

```text
canary_submit_allowed = false
live_submit_allowed = false
live_cancel_allowed = false
posted = false
cancelled = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means controlled live canary prerequisites are represented
locally and remain blocked without a future reviewed release decision. It does
not authorize a live canary.
