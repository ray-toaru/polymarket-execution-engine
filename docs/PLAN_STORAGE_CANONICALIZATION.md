# Plan storage canonicalization

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Decision

`execution_plans` is the single canonical storage location for plan summaries.

`plan_summaries` is intentionally removed before deployment. The project is still pre-live and does not need to preserve historical databases, so keeping two plan tables would create avoidable drift risk.

## Confirmed facts

- `PostgresStore::save_plan_summary()` writes to `execution_plans`.
- `PostgresStore::load_plan_summary()` reads from `execution_plans.summary_json`.
- `save_plan_summary()` is immutable: replaying the exact same plan is accepted, but the same `execution_id` with different plan JSON is rejected.
- `execution_id` is derived from the canonical plan hash prefix; the plan hash excludes `execution_id` to avoid a circular binding.
- `migrations/0001_initial.sql` now drops any pre-existing `plan_summaries` table and does not recreate it.
- `validation/check_plan_storage.py` fails if the migration recreates `plan_summaries` or if `PostgresStore` starts reading/writing it again.

## Remaining boundary

This does not make submit live-ready. It only removes schema ambiguity before implementing a funds-moving order saga.
