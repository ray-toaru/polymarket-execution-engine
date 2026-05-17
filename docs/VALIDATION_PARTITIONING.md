# Validation partitioning

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

v0.18 separates deterministic tests from environment-gated tests.

## Problem confirmed in v0.17 evidence

The uploaded v0.17 evidence contains targeted pass logs for PostgreSQL HTTP E2E, authenticated non-trading smoke, and sign-only dry-run. It also contains a generic `cargo test --workspace` log that failed because environment-gated PostgreSQL tests were included in the generic workspace run.

The root issue is validation structure, not an established business logic regression.

## v0.18 validation layout

`validation/run_current_gates.sh` runs:

1. `cargo fmt/check/clippy` for the workspace.
2. `cargo test --workspace --exclude pmx-api` for deterministic non-API workspace tests.
3. `cargo test -p pmx-api --test http_and_fake_e2e` for API tests that do not need PostgreSQL.
4. PostgreSQL migration/store/API E2E only inside the `PMX_TEST_DATABASE_URL` block.
5. Official SDK spike and adapter tests as isolated manifests.
6. Authenticated non-trading and sign-only dry-run only behind explicit environment gates.
7. Release hygiene on a clean release snapshot.

## Why not run everything in one cargo test command?

A single `cargo test --workspace` command becomes ambiguous when local developer environment variables are exported. If `PMX_TEST_DATABASE_URL` exists, PostgreSQL integration tests will run and can couple a generic code-quality gate to database lifecycle. Splitting gates makes each failure class attributable.

## Required conclusion style

- A passing targeted PostgreSQL E2E log proves PostgreSQL E2E passed for that run.
- A failed generic workspace log caused by mixed environment-gated tests does not by itself prove a business logic regression.
- A clean release hygiene result must be produced from a clean snapshot, not from a dirty working tree containing `.env`, `target/`, or local PostgreSQL data.
