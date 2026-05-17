# Real Polymarket Adapter Decision Record

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Status

SDK-first, official-source-only decision. Not live-enabled.

## Confirmed source facts, checked 2026-05-14

Primary source:

```text
https://github.com/Polymarket/rs-clob-client-v2
```

Confirmed from that repository and README:

- Repository: `Polymarket/rs-clob-client-v2`.
- Crate: `polymarket_client_sdk_v2`.
- Version observed in `Cargo.toml`: `0.6.0-canary.1`.
- Cargo metadata observed: edition `2024`, rust-version `1.88.0`.
- README describes typed CLOB requests, dual authentication flows, a type-level state machine, `alloy::signers::Signer` support including remote signers, order builders, serde support, and async reqwest design.
- README documents authenticated CLOB client construction through `Client::new("https://clob-v2.polymarket.com", Config::default())`, `authentication_builder(&signer)`, and `authenticate()`.
- README documents order build/sign/post through `market_order()` / `limit_order()`, `client.sign(&signer, order)`, and `client.post_order(signed_order)`.
- README documents WebSocket streaming through the `ws` feature and authenticated user streams.
- README states V1/V2 protocol detection is host-driven; V2 host is `https://clob-v2.polymarket.com` and V2 collateral is pUSD.

## Decision

Use the official Rust SDK as the preferred real adapter implementation behind the stable internal `pmx-gateway` traits.

```text
pmx-core: no SDK dependency
pmx-policy: no SDK dependency
pmx-store: no SDK dependency
pmx-runtime: no direct SDK dependency except future worker adapter layer
pmx-api: no direct SDK dependency
pmx-gateway: stable internal trait boundary
adapters/pmx-official-sdk-spike: isolated official SDK spike
```

## Why not raw REST first

Raw REST requires the project to own more signing, header, and signed-order payload semantics. The official SDK already documents typed request builders, authentication state transitions, signer support, and order build/sign/post operations. Therefore raw REST is not the minimal-sufficient first implementation for live signing/submission.

REST remains acceptable only for:

- read-only diagnostics,
- emergency evidence collection,
- or a documented SDK gap after a separate review.

## Important constraint: MSRV mismatch

The execution engine currently keeps a lower core MSRV baseline. The official SDK declares Rust `1.88.0` and edition `2024`. This creates a real engineering choice:

| Option | Correctness | Complexity | Maintenance | Risk | Decision |
|---|---:|---:|---:|---:|---|
| Raise entire execution engine MSRV to 1.88 | High | Low-medium | Simple | May exclude older build envs | Likely after SDK spike |
| Keep core MSRV lower and isolate SDK adapter | High | Medium | Slightly split | Requires two validation profiles | Current v0.7 approach |
| Avoid official SDK to preserve lower MSRV | Lower | High | Risky | Reimplements signing/auth details | Rejected for real adapter |

Current choice: isolate the SDK spike outside the default workspace until the project explicitly accepts Rust 1.88 for real-adapter builds.

## Attack / defense

### Round 1

Current conclusion: use only `Polymarket/rs-clob-client-v2` as the SDK source.

Defense: the repository is under the Polymarket GitHub organization and its README/Cargo metadata explicitly describe the Rust SDK crate and CLOB use cases.

Attack: GitHub main may move; canary versions can change quickly or be yanked.

Accepted attack: yes.

Revision: pin the crate version in the spike, record the source URL, and require a dependency audit before promoting the adapter into the default workspace.

### Round 2

Current conclusion: SDK adapter must remain behind `pmx-gateway`.

Defense: keeping SDK use internal prevents Hermes/control planes from seeing private keys, signed payloads, CLOB credentials, or raw SDK order objects.

Attack: the SDK type-level state machine may tempt developers to expose SDK objects directly in API responses for convenience.

Accepted attack: yes.

Revision: public OpenAPI and Python control forbidden-token scans must continue; SDK signed order objects must never cross the public API boundary.

### Round 3

Current conclusion: SDK spike must not enable live submit by default.

Defense: current evidence proves fake E2E and repository concurrency, not live exchange behavior.

Attack: read-only and sign-only smoke do not prove order posting.

Accepted attack: yes.

Revision: gates are read-only smoke, authenticated non-trading smoke, dry-run sign-only, then bounded shadow submit with kill switch and cancel-only fallback.

## Implementation gates

```text
1. v0.7 cargo gates rerun after source changes.
2. SDK spike crate typechecks with --features sdk-typecheck in Rust 1.88+.
3. Read-only SDK CLOB ok() smoke.
4. Authenticated non-trading SDK smoke.
5. PlanOrder -> SDK order builder mapping reviewed.
6. SDK sign-only dry-run with no post.
7. Signed payload remains internal and non-public.
8. Repository-level idempotency/reservation tests remain passing.
9. Runtime kill switch and cancel-only fallback verified.
10. Only then consider bounded post_order shadow smoke.
```
