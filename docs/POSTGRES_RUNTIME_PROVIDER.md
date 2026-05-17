# PostgreSQL-backed runtime provider

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.16 introduces a store-backed runtime provider boundary:

```text
ExecutorService
  -> RuntimeStateProvider
  -> StoreBackedRuntimeStateProvider<PostgresStore>
  -> RuntimeStateStore::load_runtime_state()
  -> runtime_accounts / collateral_profiles / worker_health
```

## Scope

The provider is still conservative scaffolding. It maps persisted runtime rows into a `RuntimeStateSummary` used to build feasibility snapshots.

It currently reads:

- `runtime_accounts`: account status and kill switch flag.
- `collateral_profiles`: explicit or default collateral profile status.
- `worker_health`: capability status and heartbeat freshness.

## Fail-closed rule

Missing account rows, missing required workers, explicit missing collateral profiles, stale worker heartbeat, or database errors must not produce an allow-like snapshot.

## Non-live guarantee

The runtime provider can allow a plan to reach `READY`, but submit still returns `BLOCKED`. No official SDK `post_order` path is introduced in v0.16.

## Required external tests

- `postgres_loads_runtime_state_from_runtime_tables`
- `http_postgres_runtime_rows_can_reach_ready_plan_but_submit_still_blocks`
