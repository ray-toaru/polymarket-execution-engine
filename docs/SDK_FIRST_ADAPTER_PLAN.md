# Official SDK-first Adapter Plan

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

## Current state

```text
v0.7: official SDK spike + read-only smoke evidence
v0.8: Rust baseline aligned with official SDK
v0.11: formal official SDK adapter boundary, authenticated smoke, sign-only dry-run,
plan -> builder mapping, SDK error normalization, and liveness/reconcile classification landed
```

## Promotion sequence

```text
1. SDK spike typecheck/read-only smoke: done
2. official adapter crate fmt/check/clippy/test: done
3. authenticated non-trading smoke: done
4. sign-only dry-run: done
5. plan -> SDK order builder mapping: done for LIMIT and MARKET validation boundary
6. SDK error normalization: done
7. live-submit denied-path tests
8. manual live-submit readiness review
```

## Non-negotiable constraints

```text
- no SDK dependency in core/policy/store
- no SignedOrderEnvelope in OpenAPI/Python control
- no post_order in sign-only dry-run
- no live submit without feature + env + config + runtime gates
```
