# Runtime worker model

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

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

`pmx-service::record_runtime_worker_provider_snapshot()` is the v0.25 bridge
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

`pmx-runtime::elect_heartbeat_lease_owner()` models heartbeat lease owner
election without I/O. It chooses the freshest healthy candidate, uses worker id
as a deterministic tie-breaker, and fails closed when there is no fresh healthy
owner or when the local instance is not the owner. `pmx-service::
record_heartbeat_lease_election_tick()` persists that result through the same
provider snapshot bridge, so stale lease ownership becomes a runtime blocker
before submit decisions.

`pmx-service::record_heartbeat_lease_from_worker_status()` is the persisted
heartbeat lease worker boundary. It records the local worker heartbeat into
`worker_health`, reads heartbeat-lease candidates back through
`RuntimeWorkerStatusStore`, elects the owner, writes the resulting runtime
observation, and keeps the local heartbeat capability visible as
`heartbeat-lease`. This remains local-only and non-trading: it does not open
network streams, submit, cancel, or read remote orders.

`pmx-runtime::evaluate_resource_refresh_freshness()` models the resource
refresh worker without I/O. It requires every account, market, and collateral
observation supplied by the caller to be both fresh and healthy; missing, stale,
failed, or degraded observations evaluate to `fresh=false`. `pmx-service::
record_resource_refresh_worker_tick()` turns that evaluation into the provider
snapshot bridge, so stale resource truth becomes a persisted runtime blocker
without any remote submit or cancel side effect.

`pmx-runtime::evaluate_reconcile_backlog()` models the reconcile backlog worker
without remote reads or lifecycle mutation. It accepts the caller's current
remote-unknown order ids and produces a normalized backlog count; any non-zero
count maps through `pmx-service::record_reconcile_backlog_worker_tick()` into
the same provider snapshot bridge and blocks submit as a degraded reconcile
state.

`OrderReconcileBacklogStore` and `pmx-service::
record_reconcile_backlog_from_order_lifecycle()` add the local reader side of
that worker. The reader lists account-scoped orders already in
`REMOTE_UNKNOWN` or `PARTIAL_REMOTE_UNKNOWN`, feeds their ids into the same
reconcile backlog tick, and performs no remote reads, submit, cancel, or
lifecycle mutation.

`pmx-runtime::evaluate_websocket_liveness()` models market/user WebSocket
liveness without opening sockets. It treats disconnected, stale, degraded, or
missing submit-critical channels as fail-closed inputs. `pmx-service::
record_websocket_liveness_worker_tick()` persists the resulting market/user
state through the provider snapshot bridge, keeping submit blocked until both
channels are fresh and healthy.

`pmx-runtime::evaluate_geoblock_status()` models the geoblock provider boundary
without making a provider call. Only explicit `Allowed` status is submit-safe;
`Blocked`, `Unknown`, and `Error` remain fail-closed. `pmx-service::
record_geoblock_worker_tick()` records that status through the provider snapshot
bridge so geoblock uncertainty blocks decision gates.

`pmx-runtime::evaluate_worker_crash_recovery()` models worker crash recovery
without process supervision side effects. It checks required worker
capabilities for fresh healthy heartbeats and fails closed when a required
worker is missing, stale, degraded, or failed. `pmx-service::
record_worker_crash_recovery_tick()` records both the crash-recovery worker
heartbeat and a normalized runtime observation, so recovery gaps become submit
blockers until all required workers are healthy again.

`RuntimeWorkerStatusStore` and the read-only HTTP route `/v1/runtime/workers`
expose the persisted worker heartbeats and account-scoped fail-closed
observations for shadow runtime inspection. This query path uses read-report
authorization, returns only local runtime status metadata, and has no trading
side effect.

Remaining work:

```text
- Connect concrete network providers to the deterministic worker-loop boundary.
- Connect remote reconcile readers to external order observations.
```
