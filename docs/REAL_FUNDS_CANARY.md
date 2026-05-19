# Real Funds Canary

This document defines the first real-funds test stage. It is not production-ready and it is not enabled by default.

Required gates:

- `PMX_ALLOW_LIVE_SUBMIT=1`
- `PMX_ALLOW_REAL_FUNDS_CANARY=1`
- `allow_live_submit = true`
- `allow_real_funds_canary = true`
- `compile_feature_live_submit = true`
- `kill_switch_open = true`
- `runtime_worker_healthy = true`
- `geoblock_allowed = true`
- `repository_reservation_exists = true`
- `idempotency_key_written = true`
- `reconcile_worker_healthy = true`
- `account_whitelisted = true`
- `market_whitelisted = true`
- `operator_approved = true`
- `cancel_only_fallback_ready = true`
- `balance_allowance_checked = true`
- `approval_file_required = true`
- `artifact_sha256_required = true`
- `evidence_manifest_sha256_required = true`

Canary scope:

- `REAL_FUNDS_CANARY`
- `FOK_LIMIT_FILL`
- `max_order_notional_usd = 1`
- `max_daily_notional_usd = 5`
- `auto_high_liquidity_market_selection = true`
- `max_spread_bps = 250`
- `remote_unknown_freeze_clear = true`

Safety assertions:

- `live_submit_allowed = false` during normal gates
- `live_cancel_allowed = false` during normal gates
- `real_funds_canary_allowed = false` during normal gates
- `posted = false` during normal gates
- `remote_side_effects = false` during normal gates
- `raw_signed_order_logged = false`
- `raw_signed_order_exposed = false`
- `post_order` exists only behind the `live-submit` feature and real-funds canary preconditions
- `post_orders` remains forbidden

Approval file:

- The approval file contains only operator metadata, risk caps, scope, artifact SHA-256, and evidence manifest SHA-256.
- It must not contain private keys, CLOB secrets, API secrets, raw signatures, raw signed payloads, or `SignedOrderEnvelope`.
- The example fixture is `config/real-funds-canary.approval.example.json`.

Execution policy:

- Normal validation runs only the preflight drill and must not call the SDK submit path.
- A real canary run requires a fresh artifact hash, current evidence manifest hash, explicit local approval file, and all runtime gates.
- Recovery or availability improvements must not automatically enable live submit or real-funds canary.
