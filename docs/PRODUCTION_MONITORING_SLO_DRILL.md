# Production Monitoring SLO Drill

This drill is v0.27 productionization evidence for alerting, dashboard, SLO, and
error-budget controls. It is local-only and does not claim production readiness.

Required signals:

- runtime_worker_health
- reconcile_backlog
- remote_unknown_count
- idempotency_conflict_rate
- sdk_error_rate
- audit_export_failure
- stale_worker_heartbeat
- geoblock_blocked
- postgres_unavailable

Required SLO behavior:

```text
safety_slo_breach_freezes_live_submit = true
availability_recovery_auto_enables_live_submit = false
error_budget_auto_enables_live_submit = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means local evidence represents the monitoring and SLO
decision rules required before production. It does not replace an external
dashboard, pager integration, production alert routing, or real incident review.
