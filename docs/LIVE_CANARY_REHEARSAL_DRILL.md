# Live canary rehearsal drill

This is a blocked_dry_run rehearsal for the future canary runbook. It must run
with no live submit and no live cancel capability enabled.

The rehearsal walks the local decision sequence only:

- whitelist_check
- caps_check
- operator_approval_check
- reservation_check
- idempotency_check
- reconcile_check
- remote_unknown_freeze_check
- post_submit_reconcile_check
- cancel_unknown_escalation_check
- cancel_only_fallback_check

The current rehearsal also checks the service-layer BUY size path:

- side = BUY
- size = 5 shares
- notional rule = limit_price * size
- raw signed order is not exposed
- no remote side effects occur during the rehearsal

Expected output:

```text
rehearsal_status = blocked_dry_run
posted = false
cancelled = false
remote_side_effects = false
```

This drill is not approval to run a live canary. Live submit and live cancel
remain blocked until a future reviewed release decision changes that boundary.

The release gate entrypoint remains `validation/run_current_gates.sh`; this
drill checks that the current gate chain captures
`40-live-canary-rehearsal-drill.log` in current evidence.
