# Static runtime provider

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

`StaticRuntimeStateProvider` is a deterministic test provider for `pmx-service`.

Purpose:

- prove runtime gate outcomes are controlled by server-side runtime state,
- allow tests to reach `DecisionStatus::Allow` and `PlanStatus::Ready`,
- prove submit remains blocked even when a plan is READY.

It is not a production runtime source. Production must use a PostgreSQL-backed runtime provider with geoblock, worker health, heartbeat, WebSocket liveness, collateral profile, and resource freshness checks.
