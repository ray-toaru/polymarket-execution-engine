# Production Preflight Config Fixture Drill

This drill validates both the positive and negative production preflight config
fixture paths.

Required positive fixture signals:

- fixture_secret_provider_ready = true
- fixture_operator_approval_ready = true
- fixture_alerting_ready = true
- fixture_live_submit_allowed = false
- fixture_live_cancel_allowed = false
- fixture_remote_side_effects = false

Required negative fixture signals:

- invalid_sensitive_fixture_rejected = true
- invalid_sensitive_fixture_secret_value_logged = false
- invalid_sensitive_fixture_reports_path_only = true
- forbidden_sensitive_keys_absent = false for invalid fixture
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

The invalid fixture intentionally contains a forbidden key with a fixture value.
The drill must never echo that value; it may report only the rejected field
path.
