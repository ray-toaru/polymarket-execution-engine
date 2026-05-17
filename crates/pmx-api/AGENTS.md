# AGENTS.md — pmx-api

## Scope

Applies to the HTTP/API crate.

## Rules

- Keep API behavior server-authoritative; clients may pass IDs and intent fields, but execution-critical objects must be loaded and validated server-side.
- Do not expose private keys, CLOB secrets, raw signed payloads, raw signatures, or `SignedOrderEnvelope` in responses, errors, logs, audit queries, or lifecycle events.
- Preserve service/admin token separation and startup rejection of empty or identical service/admin tokens.
- If a route, response, error, or lifecycle payload changes, update the OpenAPI file `../../openapi/executor.v1.yaml`, Hermes models, API tests, and static guards together.
- Live submit and live cancel must remain blocked unless a formally reviewed release decision changes this after full gates pass.

## Checks

Run API tests and contract validation after any change in this crate.
