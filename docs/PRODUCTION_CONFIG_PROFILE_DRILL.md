# Production Config Profile Drill

This drill is v0.27 productionization evidence for conservative production
configuration defaults. It is local-only and does not claim production
readiness.

Required defaults:

- live_submit_default_disabled
- live_cancel_default_disabled
- production_ready_default_false
- kill_switch_default_closed
- per_account_enablement_required
- per_market_enablement_required
- amount_caps_required
- operator_approval_required
- canary_profile_isolated

Required behavior:

```text
live_submit_allowed = false
live_cancel_allowed = false
production_ready_claimed = false
remote_side_effects = false
```

Passing this drill means local production config defaults are represented in
current evidence. It does not replace an external production config management
system or reviewed deployment config.
