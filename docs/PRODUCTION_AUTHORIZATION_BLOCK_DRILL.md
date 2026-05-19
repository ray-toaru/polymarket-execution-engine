# Production Authorization Block Drill

This drill is v0.27 productionization guard evidence. It proves the local
authorization matrix remains fail-closed unless every live-canary and production
promotion precondition is present. It does not authorize live submit, live
cancel, or production deployment.

The matrix uses these gates:

- compile_feature_live_submit
- env_allow_live_submit
- config_allow_live_submit
- kill_switch_open
- runtime_healthy
- geoblock_allowed
- repository_reservation_exists
- idempotency_key_written
- reconcile_healthy
- account_whitelisted
- market_whitelisted
- per_order_cap_ok
- per_day_cap_ok
- operator_approval_present
- reviewed_release_decision_present

Required scenarios:

- all_local_gates_but_no_reviewed_release
- missing_compile_feature
- missing_env_allow
- missing_config_allow
- kill_switch_closed
- runtime_unhealthy
- geoblocked
- missing_repository_reservation
- missing_idempotency_key
- reconcile_unhealthy
- account_not_whitelisted
- market_not_whitelisted
- per_order_cap_exceeded
- per_day_cap_exceeded
- missing_operator_approval

Every scenario must record:

```text
submit_allowed = false
cancel_allowed = false
posted = false
cancelled = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means the current source has executable evidence that live
capability cannot be unlocked by a partial configuration. It is still not
production-ready evidence and does not replace a reviewed canary or production
release decision.
