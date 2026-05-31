# Execution engine roadmap

> Status: v0.28 development status for the standalone Rust execution plane.
> This is not a production-ready or live-trading release decision.

## Current phase

Repeatable controlled-canary hardening while live submit, live cancel, broad
production deployment, and any second canary attempt remain blocked by default.

## Already landed for v0.28

- Real-funds canary CLI remains feature-gated and dry-run by default.
- Externally reviewed BUY/GTC post-only candidate files are validated against
  dynamic exchange-rule evidence; `target_size` is outcome shares and
  `notional_usd = limit_price * target_size` is only a risk-review value.
- Armed canary preconditions require artifact/evidence binding, a reviewed
  release decision, explicit approval, runtime-truth bindings, and no-live
  defaults everywhere else.
- Runtime truth can be supplied by a reviewed bridge file or derived from
  PostgreSQL runtime state plus scoped `CanaryRuntimeTruth` worker rows.
- Store-backed CLI preflight proves the PostgreSQL runtime-truth projection
  without posting, cancelling, exposing signed material, or logging database
  URLs.
- Armed canary stage reporting writes the latest handoff report and appends all
  post/cancel stages to `<report-file>.stages.jsonl`.
- Closeout consumes stage history, binds its SHA-256, and distinguishes normal
  order closeout from operator recovery and `post_unknown` incident recovery.
- `operator-recovery.json` closes only known-order `operator_required` states
  after readback proves the same order is canceled with no fill.
- `operator-incident-recovery.json` closes only `post_unknown` without a remote
  order id after account-level open-order, trade-history, and activity readback
  prove no matching remote order or fill was found.
- Current gates include local productionization evidence scaffolds for release
  decisions, operations, rollback, incident response, alerting/SLO, risk limits,
  deployment preflight, and external reference checks while preserving blocked
  live/prod defaults.

## Remaining before v0.28 release

1. Re-run the full current gates after all v0.28 source and documentation
   changes, including Rust, PostgreSQL, SDK adapter, credentialed non-trading,
   and sign-only dry-run evidence when the required environment is available.
2. Rebuild a deterministic v0.28 release artifact and detached sidecars only
   after the source version, compatibility matrix, release manifest, validation
   report, and release decision are coherently updated.
3. Keep any future real-funds canary attempt single-attempt scoped: fresh
   candidate, fresh reviewed `go` decision, explicit operator approval,
   immediate cancel, readback, closeout, and consumed package state.
4. Keep production/live trading out of scope unless a separate release decision
   and stronger production evidence explicitly change that boundary.

## Non-goals for v0.28

- General live trading.
- Reusable live submit/cancel enablement.
- Python-side signing or direct CLOB access.
- Production deployment approval.
- Treating the historical v0.26 canary closeout as approval for a second
  attempt.
