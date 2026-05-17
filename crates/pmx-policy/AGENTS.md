# AGENTS.md — pmx-policy

## Scope

Applies to risk, resource, lifecycle, and runtime-health policy decisions.

## Rules

- Policy changes must state whether they make the system stricter or looser.
- Loosening auth, live-submit, runtime degraded, low-resource, or redaction behavior requires explicit design justification and tests.
- Runtime `Degraded` remains fail-closed by default unless a documented release decision changes it.
- Tests should cover both allowed and blocked cases, including the strongest expected counterexample.
