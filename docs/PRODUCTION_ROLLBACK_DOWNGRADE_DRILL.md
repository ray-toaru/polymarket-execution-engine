# Production Rollback Downgrade Drill

This drill is v0.27 productionization evidence for rollback and safe downgrade
controls. It is local-only and does not claim production readiness.

Required fallback modes:

- sign-only
- cancel-only
- read-only

Required scenarios:

- sdk_failure_to_sign_only
- remote_unknown_to_cancel_only
- postgres_unavailable_to_read_only
- geoblock_to_read_only
- kill_switch_to_read_only
- recovery_requires_operator_review

Required behavior:

```text
live_submit_allowed = false
auto_reenable_live_submit = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means local rollback and downgrade decisions are represented
in current evidence. It does not replace a real deployment rollback or
production recovery review.
