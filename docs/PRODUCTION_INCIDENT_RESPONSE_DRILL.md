# Production Incident Response Drill

This drill is v0.27 productionization evidence for incident response controls.
It is local-only and does not claim production readiness.

Required incidents:

- remote_unknown
- cancel_failure
- sdk_failure
- postgres_unavailable
- geoblock
- low_resource
- worker_degraded

Required behavior:

```text
live_submit_allowed = false
remote_side_effects = false
operator_required = true
evidence_preserved = true
production_ready_claimed = false
```

Passing this drill means local incident response rules are represented in
current evidence. It does not replace a real incident drill, external pager
routing, or production incident review.
