# Production Preflight Config Guard

This guard validates the example production preflight config and any configured
`PMX_PRODUCTION_PREFLIGHT_CONFIG` file without authorizing live trading.

Required signals:

- production_preflight_config_schema_version = 1
- secret_provider_reference_present
- kms_key_reference_present
- rotation_evidence_reference_present
- break_glass_review_reference_present
- approval_id_present
- approval_hash_present
- approval_ticket_present
- approver_identity_present
- approval_expiry_present
- approval_scope_present
- alert_provider_reference_present
- alert_route_reference_present
- pager_escalation_policy_present
- dashboard_reference_present
- alert_test_evidence_present
- forbidden_sensitive_keys_absent = true
- forbidden_sensitive_values_absent = true
- references_only_no_secret_values = true
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

The config is allowed to contain references such as IDs, ARNs, URLs, ticket
identifiers, hashes, and dashboard links. It must not contain private keys, CLOB
secrets, raw signatures, raw signed payloads, or signed order envelopes.
