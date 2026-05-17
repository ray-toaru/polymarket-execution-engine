# AGENTS.md — pmx-release

## Scope

Applies to release metadata helpers and release-state modeling.

## Rules

- Do not mark `validated_release` or equivalent promotion state true unless required Rust, PostgreSQL, SDK, and local static evidence sections pass.
- Release metadata must avoid self-binding claims that cannot be true inside a zip. Use external sidecars for final artifact hashes.
- Keep release-status wording conservative unless the evidence manifest supports a stronger status.
- Keep current version numbers and release-state assertions in release manifests and decision documents, not in AGENTS.md.
