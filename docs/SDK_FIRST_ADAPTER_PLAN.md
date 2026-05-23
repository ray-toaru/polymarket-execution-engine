# Official SDK-first Adapter Plan

> Status: current v0.26.1 controlled real-funds canary source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

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
7. standard sign-only profile and plan: CLOB V2 / pUSD / deposit-wallet semantics / redacted metadata only
8. live-submit denied-path tests
9. manual live-submit readiness review
```

## Non-negotiable constraints

```text
- no SDK dependency in core/policy/store
- no SignedOrderEnvelope in OpenAPI/Python control
- no post_order in sign-only dry-run
- no live submit without feature + env + config + runtime gates
```

Current v0.25 closure:

```text
OfficialSdkStandardSignOnlyPlan validates the standard profile and maps executor
orders into SDK builder metadata while returning only a signed_order_ref
namespace. Raw signed order exposure, post_order, and cancel_order remain
forbidden.

The executor service records the standard sign-only construction through
record_standard_sign_only_construction. The service verifies execution_id,
account_id, and plan_hash against the stored plan, derives an executor-owned
redacted sign-only: reference and digest when omitted, and persists only local
sign-only lifecycle events.
```
