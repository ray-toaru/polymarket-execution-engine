# Runtime worker model

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: source landed; Rust gates pending external run.

v0.21 adds a small worker-action model around runtime signals. It does not start real network workers yet.

Signals now map to both capability health and worker actions:

```text
WebSocket signal      -> WebSocketLiveness action
HeartbeatLease signal -> HeartbeatLease action
Geoblock signal       -> Geoblock action
ReconcileBacklog      -> ReconcileBacklog action
```

Each action records:

```text
kind
capability
should_fail_closed
should_update_runtime_store
reason
```

The purpose is to make future WebSocket / heartbeat / geoblock / reconcile workers update the same runtime truth model while preserving fail-closed behavior before live submit exists.

`pmx-service::record_runtime_worker_signals()` now bridges deterministic runtime
signals into `RuntimeWorkerObservationStore`. This is still non-network
scaffolding: workers can persist observations and the existing store-backed
runtime provider can make decision gates fail closed, but no WebSocket,
geoblock, submit, or cancel side effect is started by this helper.

`pmx-service::record_runtime_worker_tick()` adds the runnable tick boundary for
worker loops: one call records `worker_health` heartbeat plus any normalized
runtime observations. WebSocket, heartbeat lease, geoblock, resource refresh,
and reconcile backlog workers should call this per tick after collecting their
own signal. The helper deliberately has no trading side effect and only updates
local runtime truth.

Remaining work:

```text
- Add real market/user/sports WebSocket workers.
- Add heartbeat lease writer.
- Add geoblock provider integration.
- Connect reconcile backlog worker to order lifecycle events.
```
