# Release hygiene clean snapshot policy

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Release hygiene checks must evaluate release contents, not a developer working tree.

Forbidden local artifacts include:

- `.env`
- `target/`
- local PostgreSQL data directories
- `__pycache__/`
- `.pytest_cache/`
- `.db`, `.sqlite`, `.sqlite3`

A developer tree may contain these during testing. That does not imply the release artifact is contaminated.

v0.18 gates create a clean snapshot before running `scripts/check_release_hygiene.py`:

- if the project is in a git repository, `git archive HEAD` is used;
- otherwise a tar snapshot is made while excluding local artifacts.

The final packaged zip must still be scanned directly with `check_release_hygiene.py <artifact.zip>`.
