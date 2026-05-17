# Production Evidence Controls

Status: governance scaffold only. This does not claim production readiness.

Production promotion is forbidden unless every control below has current
evidence for the exact artifact SHA-256 under review.

## Required Evidence

- Exact artifact binding: `evidence/current/manifest.json`, the release zip
  `.sha256` sidecar, and the `.evidence.json` sidecar must all identify the same
  artifact SHA-256.
- Full gate replay: `validation/run_current_gates.sh` must pass after the final
  code, documentation, validation, schema, and packaging changes.
- Credentialed non-trading proof: authenticated read-only smoke and sign-only
  dry-run must pass without raw signed payload exposure.
- Runtime safety proof: runtime worker health, geoblock, resource refresh,
  reconcile backlog, heartbeat lease, and crash recovery evidence must be bound
  in the manifest.
- Canary proof: live submit and live cancel canary evidence must exist, be
  explicitly reviewed, and remain scoped to whitelisted accounts, whitelisted
  markets, size caps, daily caps, operator approval, and cancel-only fallback.
- Redaction proof: public APIs, audit logs, lifecycle queries, and evidence logs
  must not expose private keys, CLOB secrets, raw signed payloads, raw
  signatures, or signed order envelopes.
- Rollback proof: kill switch, sign-only fallback, cancel-only fallback,
  read-only fallback, SDK failure, PostgreSQL unavailable, geoblock, and low
  resource drills must pass.
- Operations proof: deployment runbook, rollback runbook, incident drill,
  alerting dashboard, SLO/error budget, audit export, retention policy, and
  dependency update playbook must be reviewed and linked from the release
  decision.

## Decision Rule

If any required evidence is missing, stale, unbound to the exact artifact, or
not explicitly reviewed, the release decision must remain non-production. The
maximum permitted claim is a non-production candidate status.
