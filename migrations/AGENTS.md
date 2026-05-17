# AGENTS.md — migrations

## Scope

Applies to PostgreSQL migration files.

## Rules

- Migrations are forward-only; do not edit historical migrations without documenting compatibility impact.
- Preserve uniqueness, idempotency, advisory-lock, terminal-state, audit, and redaction invariants.
- Do not add columns intended to store private keys, CLOB secrets, raw signatures, raw signed payloads, or signed order envelopes.
- Pair migration changes with PostgreSQL validation evidence or mark PG validation as missing evidence.
