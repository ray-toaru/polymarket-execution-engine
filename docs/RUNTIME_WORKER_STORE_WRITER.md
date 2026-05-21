# Runtime Worker Store-writer Scaffold

> Status: current v0.25.0 shadow-ready SDK sign-only baseline documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.22 moves runtime worker modeling one step closer to executable workers without connecting to live streams yet.

Implemented:

- `RuntimeSignal` remains the normalized input from heartbeat, WebSocket, geoblock, and reconcile backlog observations.
- `worker_actions_from_runtime_signals()` classifies whether the signal should fail closed.
- `runtime_worker_store_writes()` produces deterministic store-write payloads.
- `RuntimeWorkerObservationStore` persists observations to `runtime_worker_observations`.
- `record_heartbeat_lease_from_worker_status()` persists local heartbeat lease
  health, reads persisted lease candidates from `worker_health`, elects the
  owner, and writes fail-closed runtime observations through the store-backed
  provider bridge. It is covered by in-memory and PostgreSQL-backed service
  tests.

Intentional non-live gaps before production promotion:

- Actual WebSocket readers.
- Actual geoblock HTTP provider.
- Actual reconcile backlog worker.
- Truth-table updates that drive `RuntimeStateProvider` in production.

Safety invariant: bad or stale runtime signals must be represented as fail-closed observations before any live submit gate can be considered.
