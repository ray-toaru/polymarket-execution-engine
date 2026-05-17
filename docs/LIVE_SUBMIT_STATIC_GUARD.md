# Live submit static guard

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Purpose

The project is still pre-live. The official SDK adapter may contain explicit safety gates for future `live-submit`, but it must not contain an actual SDK `post_order` or `post_orders` call in this release line.

## v0.19 guard

`validation/check_live_submit_guard.py` checks:

- the official SDK adapter source has no `.post_order(` or `.post_orders(` call after comments are stripped;
- the public OpenAPI contract does not expose signed/live-submit terms such as `SignedOrderEnvelope`, `signed_payload`, `private_key`, `clob_secret`, or `post_order`.

The fake gateway crate is intentionally outside the static guard because its in-memory `post_order` is a deterministic test double, not a Polymarket remote side effect.

## Limitations

This is a static guard, not a proof of absence for all future dynamic paths. It must be combined with Rust tests, OpenAPI validation, release review, and explicit runtime gates before any live adapter work.

## Required next step

The guard is wired into:

```bash
polymarket-execution-engine/validation/run_current_gates.sh
```

The expected log is:

```text
18-live-submit-static-guard.log
```
