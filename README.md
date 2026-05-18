# polymarket-execution-engine v0.25.0 shadow-ready SDK sign-only candidate

Standalone Rust execution plane for Polymarket. This package is a **shadow-ready SDK sign-only candidate**, not a production or live-trading release.

## v0.25 scope

- Server-authoritative planning and blocked submit receipt path remain in place.
- Sign-only lifecycle append/list APIs exist with local state-machine validation, PG advisory-lock scaffolding, server-assigned metadata, and `client_event_id` replay/conflict semantics.
- Execution lifecycle and admin audit query APIs expose bounded cursor-style reads.
- Runtime worker observations are aggregated into runtime state with configurable TTL via `PMX_RUNTIME_OBSERVATION_TTL_SECONDS`.
- Cancel/reconcile endpoints remain non-live and may write local lifecycle events when an `execution_id` is supplied.
- Evidence manifest scaffolding prevents claiming production or live readiness before required Rust/PG/SDK/credentialed evidence and a later release decision are present.

## Safety boundary

Live submit, live cancel, production deployment, Python signing material access, raw signed payload exposure, and CLOB secret exposure remain blocked.

## Validation

Run the full external gate when Rust and optional PostgreSQL dependencies are available:

```bash
./validation/run_current_gates.sh
```

Low-resource local checks that do not require Rust/PG:

```bash
python validation/check_plan_storage.py
python validation/check_live_submit_guard.py
python validation/check_sign_only_lifecycle.py
python validation/check_runtime_worker_models.py
python validation/check_current_evidence_manifest.py
```

Cross-repository checks such as lifecycle API parity, version consistency, Hermes model parity, and
release packaging run from the integration repository that checks out this repo alongside
`hermes-polymarket-control`.

Optional PostgreSQL and credentialed checks remain evidence-gated by `PMX_TEST_DATABASE_URL`, `PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE`, and `PMX_RUN_SIGN_ONLY_DRY_RUN`.
