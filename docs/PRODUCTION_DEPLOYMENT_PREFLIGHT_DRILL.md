# Production Deployment Preflight Drill

This drill is v0.27 productionization evidence for deployment preflight
controls. It validates local release artifact binding only. It does not deploy,
does not authorize production, and does not enable live submit or live cancel.

Required preflight inputs:

- artifact_sha256_verified
- artifact_sidecar_verified
- evidence_sidecar_verified
- evidence_manifest_sha256_bound
- migration_evidence_present
- config_diff_review_required
- config_diff_review_evidence_verified
- config_diff_review_log_hash_verified
- operator_approval_required
- live_submit_disabled
- live_cancel_disabled
- production_ready_claimed_false

Required behavior:

```text
deploy_allowed = false
live_submit_allowed = false
live_cancel_allowed = false
remote_side_effects = false
production_ready_claimed = false
```

Passing this drill means the local package, SHA-256 sidecar, evidence sidecar,
current evidence manifest, migration evidence, and production config diff review
evidence can be checked as deployment preflight inputs. It does not replace a
real deployment, external change-management approval, production config diff
review, or operator approval.
