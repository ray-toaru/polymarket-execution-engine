# AGENTS.md — OpenAPI contract

## Scope

Applies to `executor.v1.yaml` and contract-facing schema changes.

## Rules

- OpenAPI is a public contract. Keep it aligned with Rust handlers, Hermes models, tests, and docs.
- Sensitive payloads must use redacted schemas; do not add free-form response objects that can carry secrets.
- Add schema examples only when they are non-sensitive and consistent with active safety and release-status documents.
- Run `python ../scripts/validate_contracts.py` from the execution-engine directory or `python scripts/validate_contracts.py` from the repository root after changes.
