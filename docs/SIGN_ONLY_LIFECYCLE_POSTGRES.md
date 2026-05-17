# Sign-only Lifecycle PostgreSQL Persistence

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Sign-only dry-run is intentionally non-mutating. It may build/sign locally through the official SDK path, but it must never call `post_order`, never create a remote order, and never expose raw signed payloads to the control plane.

v0.22 adds a persistence boundary for this proof:

- `pmx_core::SignOnlyLifecycleRecord` remains the canonical domain record.
- `pmx_store::SignOnlyLifecycleStore` persists and lists records.
- `sign_only_lifecycle_events` stores the serialized record and enforces `no_remote_side_effect = TRUE`.
- Both in-memory and PostgreSQL stores reject records that claim a remote side effect.

Expected sequence:

```text
Planned
-> ReservationPrepared
-> SigningRequested
-> SignedDryRun
```

Failure sequence:

```text
Planned / ReservationPrepared / SigningRequested
-> Failed or Abandoned
```

This is not a live trading path. The only acceptable terminal successful sign-only state is `SignedDryRun`.
