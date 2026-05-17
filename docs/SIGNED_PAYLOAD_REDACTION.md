# Signed payload and credential redaction policy

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Status

v0.19 still has no live `post_order` path. This document covers the non-live SDK adapter boundary and all error/reporting paths that may contain SDK messages.

## Confirmed facts

- Python control plane must not receive private keys, CLOB credentials, raw signed payloads, or `SignedOrderEnvelope`.
- Sign-only dry-run may create a signed order inside the Rust adapter process, but must return only a non-reusable reference/fingerprint.
- Public OpenAPI must not expose signed payload fields.

## v0.19 changes

The official SDK adapter now includes:

- `redact_sensitive_text()` for known PMX credential environment assignments and private-key-like hex strings.
- `redact_normalized_error()` for SDK normalized errors.
- redaction in `gateway_error_from_normalized_sdk_error()` before internal gateway errors are surfaced.
- unit tests for named assignment redaction, private-key-like redaction, and gateway error redaction.

## Non-goals

This is not a complete secret-management implementation. It does not replace a production logging filter, structured tracing redaction layer, or external secret scanner. It is a local defense-in-depth layer at the SDK adapter boundary.

## Next required evidence

Run:

```bash
cd polymarket-execution-engine
./validation/run_current_gates.sh
```

Expected evidence:

```text
10-sdk-adapter-clippy.log
11-sdk-adapter-test.log
12-sdk-adapter-typecheck.log
18-live-submit-static-guard.log
19-release-hygiene-clean-snapshot.log
```
