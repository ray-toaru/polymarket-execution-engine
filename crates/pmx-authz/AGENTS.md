# AGENTS.md — pmx-authz

## Scope

Applies to authorization and token handling.

## Rules

- Authorization failures must fail closed.
- Never accept empty service/admin tokens; never allow the same configured token to satisfy both service and admin roles.
- Keep auth errors redacted and avoid echoing tokens or request secrets.
- Any widening of permissions requires a matching test proving unauthorized service/admin cross-use remains rejected.
