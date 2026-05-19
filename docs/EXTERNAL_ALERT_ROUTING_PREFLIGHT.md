# External Alert Routing Preflight

This preflight is production-readiness evidence for the external alert routing,
dashboard, and pager contract. It is a local guard for required integration
signals, not a real pager test.

Required signals:

- alert_provider_reference_present
- alert_route_reference_present
- pager_escalation_policy_present
- dashboard_reference_present
- alert_test_evidence_present
- runtime_worker_health_alert
- reconcile_backlog_alert
- remote_unknown_alert
- sdk_error_rate_alert
- audit_export_failure_alert
- pager_ack_required
- alerting_ready = false when any required signal is missing
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

An alerting configuration cannot auto-enable live trading. Missing or failing
alert routing must keep the system in read-only, sign-only, or cancel-only safe
modes until operator review completes.
