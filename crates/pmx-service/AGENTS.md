# AGENTS.md — pmx-service

## Scope

Applies to orchestration, decision compilation, sign-only lifecycle, idempotency, and service-level state transitions.

## Rules

- Preserve server-authoritative loading and binding of plan/order/market identifiers.
- Sign-only lifecycle must remain no-remote-side-effect and idempotent by `client_event_id`.
- Terminal lifecycle states must not be overwritten except by explicitly modeled, tested transitions.
- Keep fake/in-memory and PostgreSQL-backed behavior aligned; differences must be documented and covered by tests.
- Do not claim root cause or behavioral equivalence from service tests alone when store, API, or SDK evidence is missing.
