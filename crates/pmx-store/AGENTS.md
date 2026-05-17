# AGENTS.md — pmx-store

## Scope

Applies to persistence traits, in-memory store, PostgreSQL store, migrations, and lifecycle/audit persistence.

## Rules

- Preserve idempotency, advisory-lock, partial-unique-index, and terminal-state invariants for sign-only lifecycle.
- Never persist secrets, raw signatures, raw signed payloads, private keys, CLOB secrets, or signed order envelopes.
- Keep in-memory and PostgreSQL behavior semantically aligned; test both when changing repository behavior.
- Migration changes must be forward-only and paired with validation SQL or tests.
- PG concurrency claims require PostgreSQL-backed evidence; do not infer them from in-memory tests.
