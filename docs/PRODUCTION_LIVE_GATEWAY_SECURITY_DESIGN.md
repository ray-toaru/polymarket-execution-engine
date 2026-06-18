# Production Live Gateway Security Design

Status: design for future review only. This document does not enable live
submit, live cancel, production deployment, or general funds-moving execution.
Current source must remain non-live until a later reviewed release decision
binds exact code, artifact, evidence, runtime truth, operator approval, and
independent review.

Blocking summary:

- Any current claim of production live submit, production live cancel, generic
  funds-moving readiness, or reusable live gateway readiness is invalid.
- Existing official SDK live/canary runtime paths are canary/experimental
  scaffolding only. They do not satisfy this production custody design because
  they can rely on process environment secrets and in-process SDK signing.
- Current disabled, fake, sign-only, read-only, and canary-oriented code may be
  used as test scaffolding, but not as production live gateway assembly.
- A production design review must remain "required changes" until the custody,
  signed-material lifecycle, remote-unknown freeze, RBAC, cancel, market-data,
  live-read redaction, and traceability requirements below are implemented and
  independently reviewed.

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

The current official SDK runtime that reads private-key or CLOB credential
environment variables, including `POLYMARKET_PRIVATE_KEY`-style inputs, is not
an acceptable production `SecretProvider` or production signer. It may remain
only as a controlled canary, sign-only, smoke, or experimental path while live
production wiring is blocked. Production live submit binaries must not read raw
private keys from environment variables, `.env` files, profile activation
outputs, CLI flags, API requests, test fixtures, or repository files.

Production custody requires a new external signer adapter with these
properties:

- input is a secret reference and account identity, never raw key bytes;
- signing happens in KMS/HSM/external custody or in a reviewed enclave/agent
  that exposes only one-shot signing;
- credential rotation, revocation, break-glass, and audit access are tested;
- the executor receives only a one-shot opaque signed-order handle or digest
  reference with a bounded lifetime;
- no raw private key, CLOB secret, or reusable signed envelope is available to
  Hermes, the API layer, audit queries, package evidence, or logs.

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

## Production Gateway Assembly

Production gateway construction must have exactly one reviewed assembly path.
Test helpers, canary CLIs, smoke-test clients, and SDK examples must not be able
to bypass it.

Required assembly inputs:

- reviewed binary artifact SHA-256 and source commit;
- current evidence manifest SHA-256;
- release decision id and independent reviewer identity;
- production config hash and config-diff review id;
- migration ledger status;
- account and market allowlist references;
- external secret-provider readiness reference;
- alert-route readiness reference;
- rollback and incident-runbook review reference.

Startup must fail closed when any required input is missing, stale, mismatched,
or wider than the reviewed decision. Startup must also fail when:

- live features are enabled outside the approved environment;
- the same token can satisfy service and admin scopes;
- secret references resolve to raw values in process logs or config dumps;
- default/fake/test/canary gateway constructors are selected for production;
- runtime evidence is older than the configured freshness horizon;
- release artifact or manifest hashes do not match the reviewed package.

Static guards must prove that production submit/cancel can only call the
reviewed assembly path, and that tests or CLIs cannot construct a production
gateway by directly calling SDK runtime helpers.

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

### Signed Material Lifecycle

Raw signed order material must not be cached across submit stages. A production
signer must return a one-shot handle that is consumed by exactly one gateway
post attempt. The handle must be dropped, zeroized, or externally invalidated
on every exit path:

- successful post;
- pre-post runtime block after signing;
- post timeout;
- gateway rejection;
- gateway remote unknown;
- task cancellation;
- panic unwind where supported;
- signer or gateway drop.

If the implementation cannot prove cleanup for all exit paths, signing must be
moved inside the gateway's single post call so there is no sign-then-block
window. Any process-local signed-order cache, including hash maps keyed by
execution id, is forbidden for production. It may remain only in non-production
canary/test scaffolding when the release decision explicitly labels it as not
production custody.

Required tests:

- sign succeeds and the final pre-post gate blocks;
- post times out after signing;
- post returns remote unknown;
- post returns remote rejected;
- task is cancelled after signing;
- panic/drop path clears or invalidates the handle;
- cancel/readback paths cannot retrieve old signed material.

Every test must assert both lifecycle state and signed-material cleanup.

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

### Market Data And Snapshot Gate

The production submit path must bind exactly one reviewed snapshot hash and
perform a final market-data gate immediately before signing. The gate must load
or capture:

- reviewed market and condition id;
- token id and side;
- exchange-rule snapshot hash;
- top-of-book snapshot;
- tick size and minimum-size rules;
- post-only/crossing decision when post-only is required;
- freshness timestamp and source;
- liquidity and spread thresholds;
- required runtime capabilities.

`required_capabilities` from the snapshot must be translated into submit block
reasons. A missing capability, stale book, future-dated snapshot, insufficient
liquidity, exchange-rule mismatch, or market/book source mismatch blocks before
signing. A second freshness check must run after signing if signing is not
atomic with post; if that check fails, signed material cleanup is mandatory and
no remote post may occur.

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

Preconditions must map to concrete layers:

| Layer | Required production responsibility |
| --- | --- |
| API auth | Require `admin-cancel`; reject service/admin-read/release-only credentials; record subject, capability, and request correlation. |
| Service gate | Load order server-side; validate account, remote order id, lifecycle state, kill-switch/cancel-only policy, runtime freshness, and gateway readiness. |
| Store transaction | Reserve cancel idempotency by correlation id; reject reuse for a different order or action; preserve terminal-state invariants. |
| Gateway readiness | Resolve external secret reference; verify cancel endpoint readiness; call exactly one cancel; normalize timeout/rejection/unknown. |
| Reconcile | Persist readback requirement for every cancel outcome; never infer clean closure without read evidence. |

Required new interfaces before production:

- cancel idempotency reservation store keyed by account, remote order id, and
  cancel correlation id;
- cancel-only policy projection independent from submit enablement;
- gateway readiness probe that distinguishes read-only, cancel-only, and
  submit-capable modes;
- terminal-state guard that rejects cancel mutation for filled, canceled,
  failed, or closed orders unless a reviewed reconcile transition allows it;
- redacted cancel evidence record for rejection and remote-unknown outcomes.

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

### Live Read Normalization

Live readback must normalize SDK/exchange responses into allowlisted events
before storage, API exposure, or evidence export. Raw SDK responses are
forbidden outside process-local parsing.

`LiveReadNormalizedEvent` must contain only:

- `source`;
- `account_id`;
- `condition_id` or market id;
- `token_id` when applicable;
- `remote_order_id` when applicable;
- normalized `remote_status`;
- normalized fill/position/open-order quantities when applicable;
- `observed_at`;
- `freshness_expires_at`;
- `correlation_id`;
- `trace_id`;
- `redacted_error_category`;
- `redacted_error_message`.

Allowed error categories are `authentication_failed`, `rate_limited`,
`geoblocked`, `remote_unknown`, `remote_rejected`, `stale`, `schema_mismatch`,
and `internal`. Error messages must be redacted summaries and must not include
headers, tokens, query strings with secrets, request bodies, signatures, raw
signed payloads, private keys, CLOB secrets, or full exchange responses.

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

### Remote Unknown Freeze Projection

Production requires a durable freeze projection, not only lifecycle events.

Minimum schema:

```text
remote_unknown_freezes(
  freeze_id,
  account_id,
  condition_id,
  token_id,
  remote_order_id nullable,
  execution_id,
  correlation_id,
  reason_category,
  first_observed_at,
  last_observed_at,
  status,                  -- active | resolved | escalated
  resolution_evidence_hash nullable,
  resolved_at nullable,
  resolved_by_subject nullable
)
```

Submit gate logic:

1. Query active freezes by `account_id` plus market/condition/token scope.
2. Block new submit when any active freeze exists.
3. Block new submit when freeze readback is stale, inconclusive, geoblocked,
   rate-limited, or unavailable.
4. Allow submit only after a reviewed readback evidence record resolves the
   freeze and the runtime policy has no other block reason.

Resolution evidence must bind:

- freeze id;
- account and market scope;
- remote order id or no-remote-order investigation window;
- open-order readback;
- order-status readback when order id exists;
- trade/fill readback;
- account-activity readback;
- position/balance projection update;
- freshness window;
- operator/reviewer subject for manual resolution;
- hash of normalized live-read events.

Required tests:

- remote unknown creates an active account+market freeze;
- active freeze blocks submit before signing;
- stale or missing readback does not resolve the freeze;
- matching open order keeps freeze active and requires cancel/reconcile;
- missing order plus clean account-level readback resolves only within the
  reviewed investigation window;
- partial fill resolves to position/exposure update and closeout requirement;
- conflicting readback escalates and keeps submit frozen.

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

### RBAC Migration Plan

The current two-scope service/admin model is not sufficient for production
live operations. Production implementation must split authorization before
live wiring:

1. Add explicit scope or capability names for `service`, `admin-read`,
   `admin-cancel`, `release-operator`, and `emergency-operator`.
2. Split operations so `CancelOrder`, `CancelMarket`, `Reconcile`,
   `KillSwitch`, release binding, config activation, and audit export are not
   all authorized by one broad admin token.
3. Add token claims or server-side token registry metadata for subject,
   capabilities, expiry, rotation id, and environment.
4. Preserve backward-compatible non-live admin behavior only behind an
   explicit legacy profile that cannot assemble a production gateway.
5. Add audit fields for subject, scope, capability, token registry entry,
   reviewed decision id, and request correlation id.
6. Add negative tests proving service cannot cancel, admin-read cannot cancel,
   admin-cancel cannot release/enable submit, release-operator cannot cancel,
   emergency-operator cannot submit, expired tokens fail, and missing
   capabilities fail closed.

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

## Machine-verifiable Traceability Checklist

Before any real gateway can be merged as runnable production code:

| Gate | Required validation artifact | Manifest key | Freshness / expiry |
| --- | --- | --- | --- |
| Design approval | canonical design review JSON plus detached reviewer signature | `production_live_gateway_design_review` | exact commit and artifact only |
| Secret custody | external signer/KMS/HSM readiness drill and rotation drill | `production_secret_custody_validation` | expires on credential/provider/config change |
| Signed-material cleanup | unit/integration tests for one-shot handle cleanup on all exit paths | `signed_material_lifecycle_validation` | exact gateway/signer code only |
| Gateway assembly | static guard proving only reviewed assembly can build production gateway | `production_gateway_assembly_validation` | exact code/config only |
| Remote-unknown freeze | PostgreSQL tests for freeze projection and resolution evidence | `remote_unknown_freeze_validation` | exact migration/schema only |
| RBAC split | authz negative tests and token-registry proof | `production_rbac_validation` | expires on token/scope/config change |
| Cancel safety | cancel idempotency, terminal-state, readiness, and readback tests | `production_cancel_validation` | exact service/store/gateway code only |
| Market-data gate | freshness/liquidity/snapshot capability tests in final submit path | `production_market_data_gate_validation` | expires when market-data source/rules change |
| Live-read redaction | allowlist/denylist tests for normalized read events and errors | `production_live_read_redaction_validation` | exact SDK/gateway/readback code only |
| Runtime safety | stale heartbeat, degraded worker, geoblock, rate limit, auth failure, kill-switch tests | `production_runtime_safety_validation` | expires on runtime policy/config change |
| Release binding | artifact SHA-256, manifest SHA-256, provenance, migration state, config hash | `production_release_binding_validation` | exact artifact only |
| Hermes boundary | contract tests proving no signing, CLOB, DB credential, or submit/cancel direct path | `hermes_live_boundary_validation` | exact adapter/API contract only |
| Independent review | reviewer registry identity, canonical review JSON, detached signature | `independent_production_review` | exact code/config/artifact only |

The release validator must reject `production_ready=true`,
`live_trading_ready=true`, or equivalent production/live claims unless every
manifest key above is present, `pass`, fresh, exact-artifact-bound, and signed
where required. Missing keys, `skipped`, stale review, mismatched artifact hash,
or expired runtime evidence must fail closed.

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

## Audit Finding Closure Map

This revision closes the design gaps identified by
`../temp/production_live_gateway_security_design_audit.html` as design
requirements:

| Finding | Design closure |
| --- | --- |
| SEC-DES-001 | Current official SDK env-secret runtime is explicitly non-production; production requires external signer/SecretProvider. |
| SEC-DES-002 | Signed material lifecycle forbids cross-stage caches and requires one-shot cleanup tests. |
| SEC-DES-003 | Remote-unknown freeze projection, schema, gate logic, resolution evidence, and tests are defined. |
| SEC-DES-004 | RBAC migration plan splits current broad admin authority before production live wiring. |
| SEC-DES-005 | Cancel preconditions are mapped to API auth, service gate, store transaction, gateway readiness, and reconcile layers. |
| SEC-DES-006 | Final market-data/snapshot gate binds snapshot hash, freshness, liquidity, capabilities, and pre-sign/pre-post checks. |
| SEC-DES-007 | Production gateway assembly is made a single reviewed construction path with startup failure conditions and static guards. |
| SEC-DES-008 | Required gates are converted into machine-verifiable manifest keys with expiry rules. |
| SEC-DES-009 | Live readback normalization allowlist and redacted error categories are defined. |
| SEC-DES-010 | Blocking summary moved to the top and states current foundations are not live readiness. |
