# Official SDK Mapping / Error / Liveness Notes

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: v0.10 implementation note for `pmx-official-sdk-adapter`.

## Plan -> order builder mapping

```text
internal side BUY/SELL -> SDK Side::Buy / Side::Sell
internal order_kind LIMIT -> SDK limit_order()
internal order_kind MARKET -> validated but not yet executed in sign-only path
time_in_force GTC/FOK/FAK -> SDK OrderType
time_in_force GTD -> explicitly rejected in v0.10
post_only defaults false unless explicitly set
```

Current sign-only dry-run path:

```text
SignOnlyDryRunRequest
-> OfficialSdkPlanOrder
-> official_sdk_plan_to_builder_mapping()
-> SDK limit_order().build()
-> SDK sign()
-> local signed_order_ref only
```

## SDK error normalization

```text
401/403 -> AuthenticationFailed
408/429/5xx -> RemoteUnknown, retryable=true
other HTTP status -> RemoteRejected
Validation -> ValidationFailed
WebSocket -> WebSocketFailed
Geoblock -> Geoblocked
Internal/Synchronization/unknown future variants -> Internal
```

Gateway projection:

```text
AuthenticationFailed -> GatewayError::AuthenticationFailed
ValidationFailed / RemoteRejected -> GatewayError::RemoteRejected
RemoteUnknown / WebSocketFailed / Geoblocked / Internal -> GatewayError::RemoteUnknown
```

## Liveness / reconcile classification

```text
geoblock blocked -> Geoblocked
websocket disconnected -> ReconnectWebsocket
heartbeat expected but inactive -> ReconnectWebsocket
remote_unknown_orders > 0 -> ReconcileRequired
otherwise -> Healthy
```

This is classification logic only. Full WebSocket session management, heartbeat scheduling, geoblock handling, and reconcile worker wiring remain follow-up work beyond v0.10.
