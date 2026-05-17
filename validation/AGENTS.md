# AGENTS.md — validation and evidence

## Scope

Applies to validation scripts, gate runners, SQL fixtures, templates, and evidence generation.

## Rules

- `run_current_gates.sh` is the human/CI entry point; it should delegate to the current pinned version gate.
- Current evidence belongs under `evidence/current/`; historical evidence and old gates belong under archive directories excluded from release packages.
- Do not mark external Rust, PostgreSQL, SDK, or credentialed gates as passed unless the exact command ran and the log is bound in the manifest.
- Every required log entry should record command, cwd, timestamp, exit status, and enough output to audit the result.
- Update `check_docs_evidence_governance.py` and `../../scripts/check_release_artifact.py` when adding required governance files.
