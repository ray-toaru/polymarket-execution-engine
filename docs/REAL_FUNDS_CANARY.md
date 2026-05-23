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
- `runtime_kill_switch_truth_bound = true`
- `runtime_live_submit_gate_bound = true`
- `runtime_idempotency_lease_bound = true`
- `runtime_order_cancel_reconciliation_bound = true`
- `approval_file_required = true`
- `artifact_sha256_required = true`
- `evidence_manifest_sha256_required = true`

Canary scope:

- `REAL_FUNDS_CANARY`
- `GTC_LIMIT_POST_ONLY_CANCEL`
- `max_order_notional_usd = 1`
- `max_daily_notional_usd = 5`
- `target_size_is_reviewed_candidate_input = true`
- `notional_usd_is_price_times_size = true`
- `limit_order_size_driven = true`
- `runtime_truth_file_required = true`
- `runtime_truth_store_projection_available = true`
- `post_only_required = true`
- `cancel_confirmation_required = true`
- `external_candidate_market_required = true`
- `engine_market_discovery_allowed = false`
- `max_spread_bps = 250`
- `remote_unknown_freeze_clear = true`

Minimum-funds rule:

- The durable rule is minimum expected funds at risk under current exchange
  constraints, not a hard-coded share count.
- The reviewed candidate supplies `target_size` in outcome shares. The order
  uses `size = target_size`.
- `notional_usd = limit_price * target_size`; it is a derived risk-review
  value, not the order size field.
- If the exchange changes minimum size, tick, or order-mode behavior, generate
  a fresh externally reviewed candidate with a fresh `exchange_rule_snapshot`.

Safety assertions:

- `live_submit_allowed = false` during normal gates
- `live_cancel_allowed = false` during normal gates
- `real_funds_canary_allowed = false` during normal gates
- `posted = false` during normal gates
- `remote_side_effects = false` during normal gates
- `raw_signed_order_logged = false`
- `raw_signed_order_exposed = false`
- `post_order` and `cancel_order` exist only behind the `live-submit` feature and real-funds canary preconditions
- `post_orders` remains forbidden
- the armed SDK path must use `limit_order().size(...)`; `market_order().amount(...)` is forbidden for real-funds canary

Approval file:

- The approval file contains only operator metadata, risk caps, scope, artifact SHA-256, and evidence manifest SHA-256 bindings.
- `evidence_manifest_sha256` is the archived/package-sidecar manifest hash used by the armed CLI. Review packages also record `workspace_manifest_sha256` so reviewers can distinguish the raw workspace manifest from the normalized manifest embedded in the deterministic release zip.
- It must not contain private keys, CLOB secrets, API secrets, raw signatures, raw signed payloads, or `SignedOrderEnvelope`.
- The example fixture is `config/real-funds-canary.approval.example.json`.

Execution policy:

- Normal validation runs only the preflight drill and must not call the SDK submit path.
- A real canary run requires a fresh artifact hash, current evidence manifest hash, explicit local approval file, and all runtime gates.
- The armed CLI requires runtime-truth bindings for kill switch, live-submit
  gate, idempotency lease, and order/cancel reconciliation. It can consume the
  reviewed bridge file with `--runtime-truth-file`, or query PostgreSQL runtime
  truth with `--runtime-truth-store postgres`,
  `--runtime-truth-database-url-env`, and explicit
  `--runtime-truth-condition-id`. Local review evidence, operator notes, or
  environment boolean overrides alone are insufficient.
- Store-backed runtime truth now has a typed projection:
  `CanaryRuntimeTruthStore::load_canary_runtime_truth` derives kill-switch,
  live-submit gate, idempotency lease, and order/cancel reconciliation readiness
  from runtime state plus `CanaryRuntimeTruth` worker rows. Worker rows with
  matching capability but another role are ignored.
- `validation/run_real_funds_canary_store_truth_cli_preflight.py` seeds local
  PostgreSQL runtime-truth rows and runs the CLI with
  `--runtime-truth-store postgres` in `--preflight-only` mode. It proves the CLI
  can consume store-backed runtime truth without posting, cancelling, exposing a
  signed order, or printing the database URL. Use
  `--runtime-truth-output <path>` to write a references-only runtime-truth JSON
  candidate that can be checked by
  `validation/validate_controlled_canary_runtime_truth.py` before the controlled
  canary pipeline consumes it.
- The armed canary uses a GTC post-only BUY limit order and immediately cancels it. A missing cancel confirmation is a canary failure requiring manual reconciliation.
- The armed CLI writes the report file at every remote-side-effect stage. If post status is unknown, post is accepted, cancel status is unknown, cancel confirmation fails, or cancel is confirmed, the report file must contain a structured `operator_required` or stage report rather than relying on terminal output. Each stage is also appended to `<report-file>.stages.jsonl`, while `<report-file>` keeps the latest stage or final receipt for operator handoff. If the runner returns an error after recording a remote-side-effect stage, the CLI retries persistence of the last stage before surfacing the error.
- Candidate market discovery is outside the execution engine boundary. The execution engine validates an externally reviewed candidate against CLOB book/spread and risk gates. The reviewed candidate supplies the share `target_size`; `notional_usd` is only the derived `limit_price * target_size` risk value.
- Closeout requires persisted post/cancel receipt plus order-status, trade, and account-activity readback. `scripts/prepare_canary_closeout.py` turns those files into `closeout.json` and `CLOSEOUT.md` and fails if the evidence no longer supports the closeout claims.
- Risk cap comparisons use fixed decimal parsing/comparison/multiplication, not binary floating point. Invalid precision, whitespace, negative values, exponent notation, or overflow fail closed.
- Recovery or availability improvements must not automatically enable live submit or real-funds canary.
