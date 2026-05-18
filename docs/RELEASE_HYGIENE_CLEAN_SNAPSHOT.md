# Release hygiene clean snapshot policy

> Status: current v0.25.0 shadow-ready SDK sign-only baseline documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Release hygiene checks for promotion evidence must evaluate release contents, not a developer
working tree.

Forbidden local artifacts include:

- `.env`
- `target/`
- local PostgreSQL data directories
- `__pycache__/`
- `.pytest_cache/`
- `.db`, `.sqlite`, `.sqlite3`

A developer tree may contain a local `.env` during testing. Use
`scripts/check_release_hygiene.py . --dev-worktree` for that developer-only check; it permits
`.env` but still rejects caches, virtual environments, targets, and local database files. The
default directory and zip modes remain strict and reject `.env`.

Current gates create a clean snapshot before running `scripts/check_release_hygiene.py`:

- if the project is in a git repository, `git archive HEAD` is used;
- otherwise a tar snapshot is made while excluding local artifacts.

The final packaged zip must still be scanned directly with `check_release_hygiene.py <artifact.zip>`.
