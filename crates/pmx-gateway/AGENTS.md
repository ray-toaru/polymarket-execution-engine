# AGENTS.md — pmx-gateway

## Scope

Applies to exchange/gateway abstractions and live side-effect boundaries.

## Rules

- Gateway traits may model submit/cancel capabilities, but implementations must not enable live remote side effects by default.
- Any adapter bridge must make dry-run/no remote side effects / no-remote-side-effect behavior explicit and testable.
- Do not log request bodies that could contain secrets, signatures, or signed order envelopes.
- Keep fake gateway behavior close enough to real gateway semantics for state-machine tests, but do not hide live-readiness gaps.
