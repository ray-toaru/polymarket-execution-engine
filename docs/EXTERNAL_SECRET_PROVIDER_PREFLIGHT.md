# External Secret Provider Preflight

This preflight is production-readiness evidence for the external secret-provider
contract. It is intentionally a local contract guard, not a KMS, HSM, or secret
manager implementation.

Required signals:

- secret_provider_reference_present
- kms_key_reference_present
- rotation_evidence_reference_present
- break_glass_review_reference_present
- plaintext_secret_values_absent
- provider_health_check_required
- credential_rotation_required
- break_glass_review_required
- external_secret_custody_ready = false when any reference is missing
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

The preflight may observe whether provider references are configured, but it
must never print private keys, CLOB secrets, raw signed payloads, raw
signatures, or signed order envelopes. A complete provider reference set is
still not enough to authorize live trading; a future reviewed release decision
is required.

Sensitive-variable detection includes direct credential variables and
account-scoped variables such as `PMX_ACCT_*_POLYMARKET_PRIVATE_KEY`,
`PMX_ACCT_*_CLOB_SECRET`, and `POLY_API_SECRET`. The preflight may report
presence by variable name only; values must remain absent from output.
