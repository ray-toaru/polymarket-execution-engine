# Polymarket Execution Engine Architecture

Status: current v0.28.0 production-live-candidate architecture documentation.
Historical gate-specific notes are archived under `docs/archive/`; current
validation entrypoint is `validation/run_current_gates.sh`.

## Role

`polymarket-execution-engine` is a standalone deterministic execution engine. It is not a Hermes plugin and should not import Hermes code.

## Responsibilities

- Normalize intents into canonical executor semantics.
- Capture runtime feasibility snapshots.
- Evaluate constraints and risk gates.
- Compile execution plan summaries.
- Perform final pre-submit gate.
- Sign internally.
- Post/cancel through Polymarket gateway adapters.
- Maintain PostgreSQL truth for ledger, reservations, idempotency and reconcile.
- Maintain worker health and liveness state.

## Non-Responsibilities

- Strategy research.
- Natural-language reasoning.
- Human approval UI.
- Portfolio analytics outside executor reports.
- Control-plane identity management beyond service/admin token validation.

## Crates

```text
pmx-core      domain types, validation, state transitions
pmx-policy    constraint evaluation and fail-closed gates
pmx-gateway   signer and CLOB gateway traits
pmx-store     storage traits and idempotency contracts
pmx-runtime   worker role and heartbeat primitives
pmx-authz     service/admin operation scope model
pmx-api       Axum API skeleton
pmx-release   release manifest utilities
```

## Public API Contract

The OpenAPI draft lives at `openapi/executor.v1.yaml`. It intentionally excludes internal signed order structures.

## Internal-Only Objects

- `SignedOrderEnvelope`
- signer provider internals
- CLOB auth headers
- raw order payloads
- direct DB mutation APIs

## Production Truth Source

PostgreSQL only. SQLite is not part of the production design.

## Future Live Gateway Boundary

Future real gateway, production submit/cancel, and generic live readback work
must follow `docs/PRODUCTION_LIVE_GATEWAY_SECURITY_DESIGN.md`. That design is
not current production wiring and does not override the existing blocked live
submit/cancel release posture.
