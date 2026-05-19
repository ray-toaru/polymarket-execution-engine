# External Operator Approval Preflight

This preflight is production-readiness evidence for the external operator
approval workflow contract. It models the release gate inputs that a real
approval service or ticketing workflow must provide before any future live
canary can be reviewed.

Required signals:

- approval_id_present
- approval_hash_present
- approval_ticket_present
- approver_identity_present
- approval_expiry_present
- approval_scope_present
- dual_control_required
- approval_replay_block_required
- approval_expiry_enforced
- operator_approval_ready = false when any required signal is missing
- live_submit_allowed = false
- live_cancel_allowed = false
- remote_side_effects = false
- production_ready_claimed = false

The preflight must not treat a local approval-shaped value as live authorization.
Live submit and live cancel remain blocked until a future reviewed release
decision explicitly changes the release boundary.
