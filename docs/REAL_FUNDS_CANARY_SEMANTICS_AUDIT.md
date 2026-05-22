# Real Funds Canary Trading Semantics Audit

This audit covers the v0.26 controlled canary trading semantics after the
first authorized GTC post-only canary closeout. It is not a production/live
readiness claim.

## Confirmed Semantics

| Design point | Current evidence | Guard |
|---|---|---|
| Order mode | `sdk_runtime/live_canary.rs` builds `limit_order().size(...)`, `SdkOrderType::GTC`, and `.post_only(true)` | `scripts/validate_contracts.py` requires those tokens and forbids `.market_order(` in the canary runtime |
| Size meaning | `candidate-market.json` supplies `target_size`; README and `REAL_FUNDS_CANARY.md` define `size = target_size` in outcome shares | closeout script checks candidate/report target size equality |
| Notional meaning | `notional_usd = limit_price * target_size`; it is a risk cap value, not the order size field | closeout script checks Decimal equality instead of string formatting |
| Minimum-funds rule | use the smallest reviewed share size satisfying current exchange rules while staying under the reviewed notional cap | docs require regenerating candidate snapshots if exchange rules change |
| Fill closeout | readback records `CANCELED`, `size_matched=0`, zero matching trades, zero account activity, zero positions, and value `0` | closeout script fails if these readback fields stop supporting the claim |

## Corrections Made

- Replaced the stale active-document claim that the canary path used legacy
  fill-or-kill style wording.
- Replaced fixed-size rehearsal wording with reviewed candidate share-size
  wording.
- Added `scripts/prepare_canary_closeout.py` so closeout is evidence-derived
  instead of hand-written.
- Added contract validation that rejects active current canary docs describing
  the path with the old fill-or-kill wording.

## Attack Review

- If Polymarket changes minimum share size, a hard-coded size would become
  wrong. Current design treats the reviewed `exchange_rule_snapshot` and
  candidate `target_size` as fresh inputs, so the structural response is to
  regenerate and review the candidate, not alter the armed command.
- If a GTC post-only order unexpectedly matches, the runtime returns a safety
  error and closeout cannot pass because trade/account readback would no longer
  be zero.
- If `notional_usd` formatting differs (`0.10` vs `0.1`), closeout uses Decimal
  equality so formatting does not create false failures or false passes.
- If order-status-only evidence is incomplete, closeout also requires trade,
  account activity, position, and value readback. This still does not equal a
  formal exchange/account statement export.

## Residual Limits

- The closeout evidence is public/API readback plus local package evidence, not
  an exchange statement.
- The controlled canary path is not general live submit.
- Future canary attempts require a new reviewed release decision and fresh
  candidate evidence.
