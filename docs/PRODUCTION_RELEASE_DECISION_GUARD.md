# Production Release Decision Guard

This guard is v0.27 productionization evidence for release-truthfulness. It is
local-only and does not claim production readiness.

Required checks:

- release_status_not_production_ready
- release_status_not_live_ready
- validated_release_false
- production_ready_false
- live_trading_ready_false
- production_blocker_present
- live_blocker_present
- artifact_kind_source_candidate
- no_production_promotion_without_review

Required behavior:

```text
production_ready_claimed = false
live_ready_claimed = false
validated_release = false
remote_side_effects = false
```

Passing this guard means current release metadata remains truthful and cannot be
promoted by wording drift. It does not replace a future reviewed release
decision.
