# Official Polymarket SDK Integration

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Status: v0.7 SDK-first design and isolated spike. Live trading remains disabled.

## Source of truth

The official Rust SDK source for live Polymarket integration is the user-specified repository:

```text
https://github.com/Polymarket/rs-clob-client-v2
```

The execution engine treats this repository as the primary live-adapter source. GitHub-wide searching or independent REST signing is not the main path.

## Design decision

The live adapter strategy is **official SDK first**:

1. `pmx-gateway` remains the stable internal trait boundary.
2. The official SDK sits behind that boundary.
3. Python control never imports or wraps the SDK.
4. Real signing and posting remain inside Rust execution engine.
5. REST fallback is limited to read-only diagnostics unless a later evidence review proves a specific SDK gap.
6. SDK spike remains outside the default workspace until its Rust/MSRV requirements are deliberately accepted.

## v0.7 boundary changes

- `pmx-gateway` now defines `SignerProvider` and conservative signer-provider defaults.
- The official SDK spike no longer assumes a default CLOB client constructor. Its `sdk-typecheck` hook checks only that the documented CLOB client type resolves.
- A `live-submit` feature exists only as a named future gate. It must not be enabled by default.
- `allow_live_submit` remains false by default.
- `require_explicit_runtime_kill_switch_open` is true by default.

## Required integration gates

Before any live submit path is enabled:

1. SDK dependency pinned.
2. Rust/MSRV decision recorded.
3. SDK spike compiles in Rust 1.88+.
4. Read-only `ok()` smoke passes without credentials.
5. Authenticated non-trading smoke passes.
6. Sign-only dry-run passes without submit.
7. `PlanOrder -> SDK order builder -> SDK signed order` mapping is reviewed.
8. No public API exposes SDK signed order objects.
9. Kill switch and cancel-only fallback are active.
10. PG-backed idempotency, reservation, and remote-unknown tests pass on the same version.

## Explicit non-goals

- No independent EIP-712 signing implementation in the main route.
- No SDK object exposure to Hermes or other control planes.
- No live submit until shadow/dry-run gates exist.
