# Server-Authority ID-Bound API

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.14 deliberately changes decision and compile request bodies to server-issued IDs.

## Public request shape

Decision:

```json
{
  "normalized_intent_id": "norm-...",
  "snapshot_id": "..."
}
```

Compile:

```json
{
  "normalized_intent_id": "norm-...",
  "snapshot_id": "...",
  "decision_id": "...",
  "approval": {
    "approval_id": "...",
    "approved_by": "...",
    "approved_at": "...",
    "expires_at": "...",
    "approval_scope": "SHADOW",
    "approval_hash": "<64 lowercase sha256 hex>",
    "bound_artifact_sha256": "<64 lowercase sha256 hex>",
    "bound_evidence_manifest_sha256": "<64 lowercase sha256 hex>",
    "bound_snapshot_hash": "<snapshot_hash>",
    "bound_decision_hash": "<decision_hash>",
    "bound_plan_hash": null,
    "operator_identity_ref": "..."
  }
}
```

## Security rationale

Full-object payloads allowed accidental or malicious object splicing. The executor now loads objects from its own store before evaluating/compiling and verifies:

- snapshot belongs to normalized intent
- decision matches server recomputation for normalized intent + snapshot
- approval is unexpired and bound to the server snapshot hash and decision hash
- plan hash binds approval hash, snapshot hash, decision hash, order shape, executor version, and contract version
- submit plan hash matches server-authoritative plan

## Current boundary

This does not enable live submit. `SubmitRequest.mode=LIVE` fails closed until gateway posting is wired through the executor service; `BLOCKED_DRY_RUN` remains the no-remote-side-effect local lifecycle path.

## Remaining work

- Implement cancel/reconcile state machine persistence.
