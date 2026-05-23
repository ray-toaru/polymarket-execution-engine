# Real Funds Canary Closeout

This document defines the closeout boundary for the completed v0.26 controlled
canary. It does not authorize another canary, production deployment, live
submit, live cancel, or a real-funds fill target.

## Confirmed Closeout Facts

- The current controlled canary mode is `GTC_LIMIT_POST_ONLY_CANCEL`.
- The order uses `BUY/GTC` with `post_only=true`.
- `size` is the reviewed candidate share size. It is not the dollar amount.
- `notional_usd` is derived as `limit_price * size` and is used only for risk
  caps and evidence review.
- The current closeout evidence records remote status `CANCELED`,
  `size_matched=0`, zero matching trades, zero matching account activity, zero
  matching open positions, zero matching closed positions, and value `0`.

## Closeout Evidence

Generate the machine-readable and human closeout reports from the integration
repository root:

```bash
python scripts/prepare_canary_closeout.py --package-dir <exact-reviewed-go-package-dir>
```

The script reads the local canary review package, the current release zip
sidecar, order-status readback, trade readback, and public Data API account
activity readback. For v0.27 and later packages it also requires
`post-canary-report.json.stages.jsonl`, the append-only ordered stage history
written by the armed CLI beside `post-canary-report.json`. It writes:

- `closeout.json`
- `CLOSEOUT.md`

The script fails closed if the evidence no longer supports the closeout
claims. The package directory is required so multiple local review packages
cannot be confused by modification time.
The stage history must contain the accepted post stage for the same remote
order id as the final report, must not expose raw signed material, and must not
contain unresolved `operator_required` recovery state.

If the armed canary fails after a possible remote side effect, the same report
path must already contain the latest stage report. `post_unknown`,
`post_accepted`, `cancel_unknown`, and `cancel_failed` reports require operator
reconciliation before any retry or second canary can be considered.
The armed CLI also appends every stage report to `<report-file>.stages.jsonl`.
Use that JSONL file to audit the ordered post/cancel sequence; the report file
itself is the latest handoff artifact and may be overwritten by a later stage
or final receipt.

## Design Boundary

The minimum-funds canary policy is not a fixed share-count invariant.
The durable invariant is:

- use the smallest reviewed share size that satisfies current exchange rules;
- keep expected funds at risk under the reviewed per-order cap;
- use a non-crossing GTC post-only BUY limit order;
- immediately cancel after accepted posting;
- prove the result with order, trade, account activity, position, and value
  readback;
- regenerate the reviewed market candidate whenever exchange order rules
  change.

The value `5` was the current reviewed candidate share-size input because the
observed exchange rule required a minimum share size of 5 for this canary
context. It is not a permanent release constant.

## Evidence Limits

The closeout report is stronger than order-status-only evidence because it
includes trade and public Data API account activity readback. It is still not a
formal exchange/account statement export. Treat it as controlled canary
closeout evidence only, not production/live readiness evidence.
