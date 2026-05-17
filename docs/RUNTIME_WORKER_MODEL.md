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

Remaining work:

```text
- Persist worker actions to runtime tables.
- Add real market/user/sports WebSocket workers.
- Add heartbeat lease writer.
- Add geoblock provider integration.
- Connect reconcile backlog worker to order lifecycle events.
```
