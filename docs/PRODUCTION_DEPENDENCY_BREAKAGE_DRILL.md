# Production Dependency Breakage Drill

This drill is v0.27 productionization evidence for dependency update policy and
SDK upstream breakage response. It is local-only and does not claim production
readiness.

Required dependency evidence:

- exact_sdk_pin
- adapter_lockfile_present
- spike_lockfile_present
- sdk_typecheck_evidence
- sign_only_regression_evidence
- authenticated_non_trading_evidence
- rollback_plan
- compatibility_review_required
- freeze_live_submit
- downgrade_to_sign_only
- downgrade_to_read_only
- preserve_evidence

Required behavior during SDK breakage:

```text
live_submit_allowed = false
live_cancel_allowed = false
fallback_mode = sign-only
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means current evidence represents the minimum local controls
for SDK update and upstream breakage handling. It does not replace a real
upstream breakage incident, dependency update review, or external compatibility
report.
