# Single-host limited deployment

This is the single-host limited deployment scaffold for canary-readiness work.

Status: limited deployment scaffold only. This is not production-ready evidence
and does not authorize live submit, live cancel, production deployment, or a
real-funds canary fill.

This directory contains reference-only deployment templates for a single host.
The `pmx-api` unit is a long-running HTTP listener for non-live API smoke and
control-plane integration. It still does not authorize live submit, live cancel,
production deployment, or a real-funds canary fill. Production promotion remains
blocked until a future reviewed release decision binds deployment, rollback,
health, runtime-state, and external custody evidence.

- `systemd/pmx-api.service`
- `systemd/pmx-real-funds-canary@.service`
- `env/pmx-api.env.example`
- `env/pmx-real-funds-canary.env.example`
- `bin/pmx-single-host-preflight.sh`
- `bin/pmx-single-host-rollback.sh`

The templates must remain fail-closed:

- `PMX_LIVE_SUBMIT_ENABLED=0`
- `PMX_LIVE_CANCEL_ENABLED=0`
- `PMX_PRODUCTION_DEPLOYMENT_ENABLED=0`
- `PMX_ALLOW_LIVE_SUBMIT=0`
- `PMX_ALLOW_LIVE_CANCEL=0`
- `PMX_ALLOW_REAL_FUNDS_CANARY=0`

Secret material must not be stored here. Use reference-only custody metadata such
as `pass://polymarket-execution-engine/controlled-canary`; do not place private
keys, CLOB secrets, raw signatures, raw signed payloads, or signed order
envelopes in these files.

The canary systemd template runs `pmx-real-funds-canary` in `--dry-run` mode.
An armed canary attempt still requires a separately reviewed `go` release decision,
operator approval, current artifact/evidence hashes, explicit runtime gates, and
manual operator execution.
