# Server-Authority ID-Bound API

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

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
    "approval_hash": "..."
  }
}
```

## Security rationale

Full-object payloads allowed accidental or malicious object splicing. The executor now loads objects from its own store before evaluating/compiling and verifies:

- snapshot belongs to normalized intent
- decision matches server recomputation for normalized intent + snapshot
- submit plan hash matches server-authoritative plan

## Current boundary

This does not enable live submit. It only strengthens the pre-live executor service path.

## Remaining work

- Expand hash tamper rejection fixtures.
- Add PostgreSQL-backed runtime state provider.
- Persist admin audit events.
- Implement cancel/reconcile state machine persistence.
