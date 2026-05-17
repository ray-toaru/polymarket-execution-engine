# Runtime worker model

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: source landed and covered by current validation evidence.

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

`pmx-service::record_runtime_worker_provider_snapshot()` is the v0.24 bridge
from a provider snapshot to persisted runtime truth. It evaluates the pure
runtime loop, records a `runtime-worker-loop` heartbeat, persists all normalized
observations, and returns whether runtime would allow submit. A stale lease
owner, disconnected WebSocket, geoblock, stale resource refresh, or reconcile
backlog still fails closed before any submit path can proceed.

`pmx-runtime::runtime_worker_loop_tick()` is the pure worker-loop closure model.
It takes observed worker inputs for heartbeat lease owner election, market/user
WebSocket liveness, geoblock status, resource refresh freshness, and reconcile
backlog, then emits normalized `RuntimeSignal` values and fail-closed
`RuntimeWorkerAction` values. Down, stale, geoblocked, stale-resource, and
remote-unknown states block submit; recovery is allow-like only after all
required inputs are healthy.

`RuntimeWorkerProvider` and `RuntimeWorkerProviderSnapshot` define the provider
seam for real workers. Providers can read WebSocket/geoblock/resource/reconcile
state, but the snapshot must declare `no_trading_side_effect=true`; the runtime
loop consumes snapshots and never submits or cancels.

Remaining work:

```text
- Connect concrete network providers to the deterministic worker-loop boundary.
- Persist real heartbeat lease owner election from deployment runtime.
- Connect reconcile backlog worker to remote order observations.
```
