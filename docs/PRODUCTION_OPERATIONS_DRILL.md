# Production Operations Drill

This is structured v0.27 production-operations evidence for control readiness
only. It is not production-ready evidence, does not authorize a live canary, and
must run with no live submit and no live cancel capability enabled.

Scenario names emitted by the drill:

- secret_custody
- deployment_preflight
- rollback_runbook
- incident_drill
- alerting_dashboard
- slo_error_budget
- audit_export_retention
- risk_limits
- dependency_sdk_breakage

The drill records conservative fallback modes only:

- read-only
- sign-only
- cancel-only

Expected output:

```text
status = pass
production_ready_claimed = false
live_submit_env_enabled = false
live_cancel_env_enabled = false
remote_side_effects = false
```

Passing this drill means the v0.27 operations control inventory is represented
as current evidence. It does not replace reviewed secret-manager/KMS/HSM
implementation, external alerting dashboards, real incident drills, real live
canary submit/cancel evidence, or a reviewed production release decision.
