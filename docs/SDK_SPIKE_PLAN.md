# SDK Spike Plan

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Objective

Prove that the official Polymarket Rust SDK can be used behind `pmx-gateway` without weakening execution-plane boundaries.

## Step 1: typecheck only

```bash
cargo test --manifest-path adapters/pmx-official-sdk-spike/Cargo.toml --features sdk-typecheck
```

Evidence required:

- crate resolves
- CLOB client type resolves
- no live network call
- no signer material required

## Step 2: read-only smoke

Add a test that calls the official SDK read-only health/ok endpoint, with no credentials and no order building.

Evidence required:

- endpoint reachable
- response parsed
- no private key or L2 API credential loaded

## Step 3: authenticated non-trading smoke

Use test credentials only to verify authenticated non-trading operations such as account identity or permissions. No order signing or posting.

Evidence required:

- credential provider is executor-owned
- Python control does not receive credentials
- failure modes are fail-closed

## Step 4: sign-only dry run

Build and sign an order payload without posting it.

Evidence required:

- signed payload remains internal
- `SignedOrderEnvelope` is stored only as an internal reference
- public API receives only summary/receipt

## Step 5: guarded post-order shadow

Only after repository-level idempotency/reservation and runtime gates pass on the same version.

Evidence required:

- kill switch open explicitly
- account budget tiny and explicit
- cancel-only fallback tested
- remote-unknown recovery tested
