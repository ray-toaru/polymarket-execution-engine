# PostgreSQL Concurrency Model

> Status: current v0.26.0 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Evidence status

External validation evidence confirms the PostgreSQL advisory-lock primitive: two sessions contending on the same lock key serialized, with the B-side waiting about 15.5 seconds before acquiring the lock.

This proves the lock mechanism, not the full application repository.

Repository-level PostgreSQL tests now cover core local invariants:

- same request replay for submit idempotency;
- fingerprint mismatch conflict;
- same resource reservation contention;
- remote-unknown conservative persistence;
- sign-only lifecycle `client_event_id` replay under concurrent writers;
- sign-only lifecycle mismatch rejection and terminal-state rejection.

## Intended submit transaction shape

```text
BEGIN;
  derive account_id from execution_plan
  derive resource lock key from (namespace, account_id, execution_id or resource scope)
  pg_advisory_xact_lock(lock_key)
  read execution_plan / runtime state / idempotency record
  if matching idempotency response exists: replay response and COMMIT
  if matching identity but different request_fingerprint: return conflict and ROLLBACK
  insert or continue idempotency_records row
  reserve order-level resource
  record saga attempt state
COMMIT;

remote post/signing step is never hidden by the lock proof; remote side effects still require
saga state and reconcile handling.
```

## Lock key rule

`pmx-store::advisory_lock_key(namespace, account_id, resource_key)` is a deterministic helper for app-side key generation. SQL uniqueness constraints remain the correctness backstop; advisory locks are contention control, not the only invariant.

## What remains unproven

- No serialization failure/retry loop has been exercised.
- No remote side-effect recovery path has been tested against PostgreSQL.
- Cancel pending/confirmation transition under concurrent reconcile still needs a repository-level PostgreSQL proof.

## Covered repository proof gates

```text
same request replay
fingerprint mismatch conflict
same resource reservation contention
remote-unknown conservative persistence
sign-only lifecycle concurrent client_event_id replay
```

## Next proof gate

Before any real Polymarket adapter is connected, run repository-level concurrency tests over a real PostgreSQL instance:

```text
cancel pending/confirmation transition under concurrent reconcile
```
