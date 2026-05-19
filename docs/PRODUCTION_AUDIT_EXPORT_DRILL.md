# Production Audit Export Drill

This drill is v0.27 productionization evidence for redacted audit export and
retention controls. It is local-only and does not claim production readiness.

The exported record shape must include:

- trace_id
- order_id
- client_event_id
- signed_order_ref
- signed_order_digest
- lifecycle_state
- retention_policy_id
- export_batch_id
- legal_hold
- access_reviewed

The exported record shape must not include:

- private_key
- clob_secret
- raw_signed_payload
- raw_signature
- SignedOrderEnvelope

The drill also checks:

- immutable_export = true
- redacted_export = true
- deletion_policy_defined = true
- retention_duration_days is positive
- export_failure_blocks_promotion = true
- remote_side_effects = false
- production_ready_claimed = false

Passing this drill means the local audit export contract is represented in
current evidence. It does not replace an external immutable export store,
retention-system integration, access-review workflow, or legal-hold
implementation.
