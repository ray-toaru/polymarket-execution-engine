# Rust module governance plan

This plan is the v0.25 structure-governance entry point. It is intentionally
behavior-preserving and must not introduce live submit, live cancel, signing
material exposure, or production-readiness claims.

## Entry criteria

- Current validation-promotion evidence is present under `evidence/current/`.
- Shadow-readiness drills have passed or are explicitly skipped with reason.
- No required gate is failing.

## Split order

1. Pure DTOs, enums, and error types.
2. Repository traits before implementation modules.
3. PostgreSQL implementations after trait boundaries are stable.
4. SDK adapter config, signer boundary, transport, dry-run, and error mapping.
5. Service orchestration only after lower-level modules are stable.

## Current progress

- First behavior-preserving split batch moved sign-only lifecycle and standard
  construction into separate `pmx-service::sign_only` submodules.
- PostgreSQL admin audit and execution lifecycle persistence were separated
  under `pmx-store::postgres_audit` implementation modules.
- SDK adapter plan mapping now separates normalization and validation helpers
  under `pmx-official-sdk-adapter::mapping` without changing public exports.
- SDK liveness now keeps SDK error normalization behind a feature-gated
  `liveness::error_normalization` module while preserving the public function.

## Per-step rules

- One small module move per commit.
- No semantic changes during a move.
- No OpenAPI or Hermes contract changes unless the public API actually changes.
- No new dependencies unless justified by the moved boundary.
- Keep old tests passing before moving to the next split.

## Required checks after each split

```bash
cargo fmt --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
python validation/check_live_submit_guard.py
python validation/check_sign_only_lifecycle.py
python validation/check_runtime_worker_models.py
```

From the integration repository, also run:

```bash
python scripts/validate_contracts.py
python polymarket-execution-engine/validation/check_docs_evidence_governance.py
```

## Stop conditions

- Any behavior changes without explicit design approval.
- Any redaction, runtime fail-closed, lifecycle idempotency, or live-submit guard regression.
- Any uncertainty about whether a move changes public API behavior.
