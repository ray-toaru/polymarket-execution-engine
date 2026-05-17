# Sign-only lifecycle persistence

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: source landed; Rust gates pending external run.

The sign-only path is still non-mutating. It may build and sign an SDK order only under explicit sign-only gates, but it must not call live submit, live cancel, or persist a `Posted` order.

v0.21 adds a core lifecycle trace for sign-only dry-runs:

```text
Planned
-> ReservationPrepared
-> SigningRequested
-> SignedDryRun
```

Every record carries `no_remote_side_effect = true`. A receipt that claims `posted = true` is rejected by the official SDK adapter helper.

This gives later store-backed integration a safe set of records to persist through the existing execution lifecycle mechanism without introducing remote Polymarket side effects.

External validation required:

```bash
cd polymarket-execution-engine
./validation/run_current_gates.sh
```
