# AGENTS.md — pmx-core

## Scope

Applies to shared domain types, identifiers, errors, lifecycle enums, order/plan models, and canonical serialization helpers.

## Rules

- Treat type changes as cross-crate contract changes. Update API, store, service, Hermes models, docs, and tests together when public fields or enum variants change.
- Preserve deterministic serialization for canonical IDs, plan storage, lifecycle events, and idempotency keys.
- Do not add sensitive fields to public domain events unless they are explicitly redacted before persistence and query.
- Prefer new explicit enums or structs over loosely typed maps for lifecycle, audit, and policy payloads.
