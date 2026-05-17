# Next external gates

> Status: current v0.23.0 source-candidate documentation. Historical gate-specific notes are archived under `docs/archive/`; current validation entrypoint is `validation/run_current_gates.sh`.

Run from `polymarket-execution-engine` in an environment with Rust 1.88, Cargo, rustfmt, clippy, and optionally PostgreSQL:

```bash
./validation/run_current_gates.sh
```

Expected evidence directory:

```text
evidence/YYYY-MM-DD/v0.23/
```

Core evidence logs that must exist before calling this a validated release:

```text
01-cargo-fmt.log
02-cargo-check.log
03-cargo-clippy.log
04-cargo-test-workspace-non-api.log
05-http-fake-e2e.log
06-sdk-spike-no-features.log
07-sdk-spike-typecheck.log
08-sdk-adapter-fmt.log
09-sdk-adapter-check.log
10-sdk-adapter-clippy.log
11-sdk-adapter-test.log
12-sdk-adapter-typecheck.log
18-plan-storage-guard.log
19-live-submit-static-guard.log
20-sign-only-lifecycle-guard.log
21-runtime-worker-model-guard.log
22-v0-23-lifecycle-api-guard.log
23-version-consistency-guard.log
24-contract-validation.log
25-release-hygiene-clean-snapshot.log
26-package-release.log
27-release-artifact-check.log
```

PostgreSQL evidence is required for PG-backed validation and requires `PMX_TEST_DATABASE_URL`:

```text
13-pg-migration.log
14-pg-store-tests.log
15-http-postgres-e2e.log
```

Optional credentialed non-trading evidence must only run when explicitly enabled:

```text
16-authenticated-smoke.log
17-sign-only-dry-run.log
```

Current boundary: until these logs are produced and reviewed, v0.23 remains a source candidate, not a validated release. Live submit and live cancel remain disabled.
