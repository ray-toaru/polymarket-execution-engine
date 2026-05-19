# Production Preflight Config Diff Review

This guard validates the deployment preflight config diff review contract.

Inputs:

- PMX_PRODUCTION_PREFLIGHT_BASELINE_CONFIG
- PMX_PRODUCTION_PREFLIGHT_CANDIDATE_CONFIG

Required signals:

- config_diff_review_passed = true for valid reference-only changes
- config_diff_review_rejected_sensitive_candidate = true
- config_diff_review_secret_value_logged = false
- config_diff_review_reports_path_only = true
- config_diff_summary_uses_hashes = true
- changed_field_paths_present = true
- baseline_config_hash_present = true
- candidate_config_hash_present = true
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

The diff summary may include changed field paths and SHA-256 hashes of the
baseline and candidate configs. It must not print full reference values, private
keys, CLOB secrets, raw signatures, raw signed payloads, or signed order
envelopes.
