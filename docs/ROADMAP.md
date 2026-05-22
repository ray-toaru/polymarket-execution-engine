# Execution engine roadmap

> Status: current v0.26.0 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Current phase

Real-funds canary program-readiness without live execution.

## Next

1. Rebind `evidence/current/` to the currently pinned execution-engine source.
2. Keep normal gates no-live: live submit, live cancel, and real-funds canary execution remain disabled.
3. Add a local-only real-funds canary CLI that defaults to dry-run and is compiled only with `live-submit`.
4. Add SDK read-only automatic market selection with fail-closed depth, spread, and market-state checks.
5. Add readiness validation proving the CLI cannot arm without explicit env/config/approval/artifact/evidence gates.
6. Only after a later reviewed release decision and fresh evidence, run an actual small FOK real-funds canary.
