# Official SDK Adapter Boundary

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: v0.11 adds the formal adapter crate boundary under `adapters/pmx-official-sdk-adapter`.

## Decision

The official Polymarket Rust SDK is the primary and only planned live CLOB adapter path. REST self-signing is not a primary route for signing or posting orders.

The adapter remains outside the default workspace until the following are proven:

```text
1. v0.11 workspace gates pass on Rust 1.88+
2. SDK spike gates pass
3. official adapter crate gates pass
4. authenticated non-trading smoke passes
5. sign-only dry-run passes
6. live-submit safety gates are reviewed and manually enabled
```

## Dependency boundary

Allowed dependency direction:

```text
pmx-core        <- no SDK
pmx-policy      <- no SDK
pmx-store       <- no SDK
pmx-runtime     <- no SDK by default
pmx-gateway     <- traits only, no SDK
pmx-official-sdk-adapter -> SDK dependency behind explicit feature flags
```

Forbidden:

```text
Python control importing SDK types
OpenAPI exposing SDK signed order payloads
pmx-core containing SDK order types
pmx-store containing SDK response types as schema truth
post_order reachable without live-submit feature and runtime PMX_ALLOW_LIVE_SUBMIT=1
```

## Why this is not over-design

The execution engine is a funds-moving system. The separation prevents a convenience SDK import from weakening the already-established control-plane/execution-plane boundary.
