# AGENTS.md — pmx-runtime

## Scope

Applies to runtime state, worker heartbeat, capability health, reconciliation, and resource-refresh models.

## Rules

- Runtime state must fail closed when heartbeat, reconcile, resource-refresh, or required capabilities are missing, stale, failed, or degraded under the active safety policy.
- Store-backed runtime state must remain compatible with static/fake providers used in tests.
- Any TTL, status, or capability semantic change requires tests for fresh, stale, failed, degraded, and missing states.
- Do not use runtime health to imply live readiness unless external gates and release evidence support it.
