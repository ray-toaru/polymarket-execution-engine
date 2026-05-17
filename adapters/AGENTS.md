# AGENTS.md — SDK adapters

## Scope

Applies to adapter crates and SDK spike code under `polymarket-execution-engine/adapters/`.

## Rules

- Adapter and spike code must default to no remote side effects.
- Do not enable live submit, live cancel, or credentialed actions without explicit env gates and release approval.
- Never commit private keys, CLOB secrets, API keys, raw signed payloads, raw signatures, or signed order envelopes.
- Sign-only dry-run may produce references only when explicitly gated and redacted.
- If SDK API mapping changes, update `../docs/SDK_MAPPING_AND_LIVENESS.md`, adapter tests, and evidence expectations.

## Checks

Run adapter fmt/check/clippy/tests when Rust tooling is available. Without credentials, keep credentialed smoke marked skipped or not run.
