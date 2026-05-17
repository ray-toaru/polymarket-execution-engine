# Live Canary Blocked Drill

This drill is the current v0.26 canary execution shell. It must remain blocked
unless a future reviewed release decision explicitly authorizes live side
effects.

Current expected result:

```text
canary_status: blocked
posted: false
cancelled: false
remote_side_effects: false
```

The drill fails if `PMX_ALLOW_LIVE_SUBMIT=1`, `PMX_ALLOW_LIVE_CANCEL=1`, or
`PMX_OPERATOR_APPROVED_LIVE_CANARY=1` is present. A future real canary must add
new evidence rather than editing this blocked result into success.
