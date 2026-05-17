# Authenticated Non-trading SDK Smoke

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: executed on 2026-05-15 in a credentialed environment with explicit gates. Evidence: `evidence/2026-05-15/v0.10/14-authenticated-smoke.log`.

## Purpose

Validate the official Polymarket Rust SDK authentication path without creating orders, signing orders, posting orders, cancelling live orders, or mutating remote trading state.

The official SDK documents normal authenticated flow through `Client::new(...).authentication_builder(&signer).authenticate().await`, and supports typed CLOB requests, authentication flows, signer support, order builders, WebSocket, and heartbeats. The execution engine uses the SDK as the only intended trading adapter path, while keeping it isolated from core state-machine crates.

## Required environment

```bash
export PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE=1
export POLYMARKET_PRIVATE_KEY=...
export POLY_API_KEY=...
export POLY_API_SECRET=...
export POLY_API_PASSPHRASE=...
```

## Allowed calls

Allowed examples:

```text
ok()
server_time()
readonly_api_keys()
closed_only_mode()
balance_allowance()
```

Disallowed:

```text
market_order()
limit_order()
sign(...)
post_order(...)
post_orders(...)
cancel_order(...)
cancel_orders(...)
update_balance_allowance(...)
anything that writes remote state
```

## Acceptance criteria

```text
- no order is built
- no signed order is materialized
- no remote trading mutation is called
- all sensitive environment presence is logged as boolean only
- raw secret values are never logged
- timeout is enforced per remote call
```

## Boundary

Passing this smoke proves only that authentication can be established for non-trading reads. It does not prove order signing, order posting, cancellation, reservation correctness, reconcile correctness, or production readiness.
