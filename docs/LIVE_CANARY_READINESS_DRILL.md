# Live Canary Readiness Drill

This drill validates the canary gate model without enabling live submit or live
cancel. It is a readiness check only; it does not place orders, cancel orders,
sign live payloads, or call Polymarket trading endpoints.

Required gates before a future canary:

- compile feature for live submit;
- environment gate for live submit and live cancel;
- config gate for live submit;
- kill switch open;
- runtime worker healthy;
- geoblock allowed;
- repository reservation exists;
- idempotency key written;
- reconcile worker healthy;
- account and market whitelist;
- size cap and daily cap;
- operator approval;
- cancel-only fallback ready.
- remote unknown freeze before any canary attempt.

`prepare_live_canary_decision()` builds the local canary-prep decision from
account whitelist, market whitelist, per-order cap, per-day cap, operator
approval, cancel-only fallback, and remote-unknown backlog inputs. The prep
decision never enables submit by itself and records `live_side_effects=false`.

Validation entrypoint:

```bash
python validation/run_live_canary_readiness_drill.py
```

The script must fail if `PMX_ALLOW_LIVE_SUBMIT=1` or `PMX_ALLOW_LIVE_CANCEL=1`
is present in the validation environment. Passing this drill means the gate model
is present and current source remains no live submit / no live cancel; it is not
approval to run a live canary.
