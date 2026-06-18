# Production Live Gateway Security Design

Status: design for future review only. This document does not enable live
submit, live cancel, production deployment, or general funds-moving execution.
Current source must remain non-live until a later reviewed release decision
binds exact code, artifact, evidence, runtime truth, operator approval, and
independent review.

## Scope

This design covers the future production path for:

- a real Polymarket CLOB gateway implementation;
- production `submit` and `cancel` operations;
- generic live readback for open orders, order status, trades, fills,
  positions, account activity, exposure, and reconciliation inputs.

It deliberately excludes strategy logic, Hermes signing, Hermes direct CLOB
access, secret storage in source files, reusable approval packages, and any
automatic retry that can create a second funds-moving attempt without a new
review.

## Required Architecture

The only allowed production authority layout is:

```text
Hermes/control plane
  -> typed executor API request with IDs and intent references
  -> Rust execution service
  -> PostgreSQL truth and policy/runtime gates
  -> signer provider boundary
  -> real CLOB gateway
  -> readback/reconciliation workers
```

Hermes and the Python adapter remain request/report clients. They must never
hold private keys, CLOB API secrets, raw signed payloads, raw signatures,
signed order envelopes, executor database credentials, or direct CLOB clients.

The Rust execution engine owns the server-authoritative object graph:

- active account and profile identity;
- reviewed plan and plan hash;
- candidate market and exchange-rule snapshot hash;
- approval and independent-review binding;
- risk limits and runtime truth;
- idempotency lease and submission correlation;
- lifecycle state, readback, reconciliation, and audit records.

## Trust Boundaries

### Secret Boundary

The production signer and gateway may receive secret references, not raw
secrets from the API or Hermes layer. Secret resolution must be performed by a
reviewed `SecretProvider` implementation backed by an external secret manager,
KMS, HSM, or other approved custody system.

Forbidden in logs, API responses, audit rows, lifecycle events, package
evidence, and panic/error strings:

- private keys;
- CLOB secrets and HMAC preimages;
- raw signed order payloads;
- raw signatures;
- reusable signed order envelopes;
- full auth headers or bearer tokens.

### Side-effect Boundary

Live side effects are allowed only through a dedicated real gateway assembly
that is absent from default builds and default runtime profiles. The disabled,
fake, sign-only, and read-only adapters remain the default wiring.

Production submit/cancel requires all of the following to be true at the same
time:

- compile-time live feature is enabled for the reviewed binary;
- runtime live profile is explicitly enabled for one account and environment;
- kill switch is clear;
- runtime workers are healthy and fresh;
- PostgreSQL lifecycle and idempotency stores are reachable;
- secret provider and signer readiness pass;
- market, account, amount, exposure, geoblock, and stale-book policies pass;
- exact release artifact, evidence manifest, config diff, and migration state
  match the reviewed decision;
- operator approval and independent review are current and unconsumed.

Failure or uncertainty in any item must block new submit. Cancel-only and
read-only recovery can remain available only when the release decision and
runtime policy explicitly permit them.

## Production Submit Design

Submit is a multi-stage transaction with a single remote side-effect boundary.

```text
Load reviewed plan
  -> validate account/market/risk/runtime/fresh-book gates
  -> reserve idempotency lease in PostgreSQL
  -> record pre-sign lifecycle event
  -> resolve signer by secret reference
  -> sign in process-local memory
  -> record signed-order digest reference only
  -> re-check kill switch and runtime truth
  -> post through real gateway
  -> persist remote ack or remote-unknown outcome
  -> trigger readback/reconcile requirement
```

The gateway may never persist or return raw signed material. Receipts expose
only redacted IDs, digest references, lifecycle state, and operator action
requirements.

Submit outcomes:

| Outcome | Required behavior |
| --- | --- |
| Remote accepted | Persist remote order id, mark readback required, keep cancel path available. |
| Remote rejected | Persist rejected terminal event with redacted reason; do not retry automatically. |
| Remote unknown | Freeze new submit for the account/market, require readback and operator recovery. |
| Local failure before post | Persist local failed/pre-post state; no remote recovery claim is allowed. |
| Runtime degraded after ack | Keep posted state, return partial remote unknown, require operator review. |

Automatic post retry is forbidden unless a later design proves idempotent
remote semantics for the exact SDK/CLOB endpoint and receives independent
review. Until then, remote unknown means no second submit.

## Production Cancel Design

Cancel is a separate admin-scoped side effect and must be safer than submit.
It can be allowed in cancel-only recovery mode when new submit is frozen.

Cancel preconditions:

- target order is loaded server-side from PostgreSQL;
- account and remote order id match persisted lifecycle truth;
- caller has admin/cancel capability;
- kill switch policy allows cancel or cancel-only recovery;
- gateway and secret provider readiness pass;
- idempotency/correlation id has not been reused for a different cancel.

Cancel outcomes:

| Outcome | Required behavior |
| --- | --- |
| Cancel confirmed | Persist cancel-confirmed event and require readback closure. |
| Already missing/closed | Persist reconcile evidence; do not treat as clean without readback. |
| Partial fill before cancel | Persist fill evidence, update exposure/position projections, require closeout. |
| Cancel rejected | Persist redacted rejection and escalate to operator. |
| Cancel remote unknown | Keep order in operator-required state; continue read-only reconciliation; do not reopen submit. |

Cancel must not overwrite terminal filled/canceled/failed states except through
explicit, reviewed reconciliation transitions.

## Generic Live Read Design

Readback is live-capable but read-only. It must be available as a separate
capability so production can degrade from submit/cancel to read-only recovery.

Required read domains:

- single order status by remote order id;
- account open orders;
- trades/fills;
- positions and balances;
- account activity;
- market book and tick/size rules;
- gateway health, geoblock, rate-limit, and SDK liveness signals.

Readback records must be normalized into redacted domain events and projections.
They must bind source, account, market, remote order id, observed timestamp,
request correlation, and freshness horizon. Stale, missing, or conflicting
readback is not success; it is reconciliation input.

Production read APIs can expose redacted state and projections. They must not
expose gateway credentials, raw exchange responses containing sensitive
headers, signed payloads, or unreviewed secret references.

## State And Reconciliation Rules

Remote unknown freezes submit for the affected account/market until resolved.
Resolution requires readback evidence proving one of:

- no matching remote order/fill/activity exists in the reviewed window;
- the order exists and is open;
- the order was canceled;
- the order filled or partially filled and projections were updated;
- the remote state remains unknown and operator escalation stays active.

The reconciliation worker must be idempotent and must not create side effects
other than local store writes and alerts. It must never infer "safe to trade"
from absence of data when the data source is stale, rate-limited, geoblocked,
or otherwise degraded.

## Authorization Model

Minimum production roles:

- service: prepare, read redacted state, request non-live operations;
- admin-read: inspect audit, runtime, and reconciliation state;
- admin-cancel: run cancel or cancel-only recovery;
- release-operator: bind reviewed release/config/runtime decisions;
- emergency-operator: activate kill switch and read-only/cancel-only modes.

Service credentials must not satisfy admin scopes. Empty tokens, identical
service/admin tokens, and missing capability claims must fail startup or
request authorization.

## Configuration Model

Production live configuration must be positive and narrow:

- default `live_submit=false` and `live_cancel=false`;
- per-environment live enablement;
- per-account allowlist;
- per-market allowlist;
- per-side and per-order-type allowlist;
- per-order, per-day, and exposure caps;
- stale-book freshness threshold;
- max remote-unknown count of zero for new submit;
- mandatory alert routes;
- external secret references only.

Configuration changes that widen authority require config-diff review,
independent review, and CI/evidence refresh for the exact artifact.

## Observability And Incident Response

Required alerts:

- live gate unexpectedly enabled;
- kill switch changed;
- remote unknown observed;
- cancel unknown observed;
- runtime worker stale/degraded;
- PostgreSQL unavailable;
- secret provider or signer unavailable;
- gateway auth failure;
- geoblock/rate-limit/SDK liveness failure;
- stale market book;
- exposure or daily cap breach;
- audit export failure.

Incident modes:

- freeze submit;
- cancel-only recovery;
- read-only recovery;
- secret rotation;
- SDK downgrade or gateway disable;
- forward-fix database migration;
- independent post-incident review before unfreeze.

## Required Implementation Gates

Before any real gateway can be merged as runnable production code:

1. Separate design review approves this document or a successor.
2. Real gateway code is isolated behind a compile-time feature and runtime
   profile that is disabled by default.
3. Static guard proves no default path can call real submit/cancel.
4. Redaction tests prove no raw secret or signed material reaches logs,
   receipts, public models, audit rows, or package evidence.
5. PostgreSQL tests cover idempotency, concurrent submit/cancel, remote
   unknown, partial fill, terminal-state protection, and reconciliation.
6. Runtime tests cover stale heartbeat, degraded worker, stale book, geoblock,
   rate limit, gateway auth failure, signer failure, and kill switch.
7. Readback tests cover open, missing, canceled, filled, partial fill, stale,
   conflicting, and remote-unknown observations.
8. Release validator rejects production/live claims unless the live evidence
   sections pass for the exact artifact.
9. Hermes contract tests prove the adapter cannot sign, submit/cancel directly,
   hold executor DB credentials, or call CLOB.
10. Independent reviewer signs the exact code, config, artifact, runtime truth,
    operator runbook, and rollback plan.

## Current Non-live Boundary

The current codebase already models many ports and lifecycle states needed by
this design, including disabled/fake gateway implementations, signer-provider
boundaries, secret-provider ports, non-live lifecycle persistence, runtime
health, market-book freshness checks, portfolio/risk projections, and
read-only reconciliation shapes.

Those foundations are not live readiness. The missing production review items
remain: real gateway custody design, live runtime assembly, production
configuration, full PostgreSQL/live-read evidence, alerting proof, incident
drills, exact artifact review, and explicit release decision.
