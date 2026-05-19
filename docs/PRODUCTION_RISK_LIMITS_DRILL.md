# Production Risk Limits Drill

This drill is v0.27 productionization evidence for account, market, amount, and
exposure risk limits. It is local-only and does not claim production readiness.

Required checks:

- account_whitelist
- market_whitelist
- per_order_cap
- per_day_cap
- exposure_cap
- operator_approval_threshold
- remote_unknown_freeze_override
- stale_market_data_blocks
- geoblock_blocks

Required behavior:

```text
live_submit_allowed = false
remote_side_effects = false
operator_required = true
production_ready_claimed = false
```

Passing this drill means local risk-limit decisions are represented in current
evidence. It does not replace real account/market risk approval, treasury
limits, or strategy review.
