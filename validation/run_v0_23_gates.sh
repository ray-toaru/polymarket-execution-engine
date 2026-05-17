#!/usr/bin/env bash
set -euo pipefail

# Current release entrypoint: validation/run_current_gates.sh delegates here.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INTEGRATION_ROOT="${PMX_INTEGRATION_ROOT:-$(cd "${ROOT}/.." && pwd)}"
EVIDENCE_ROOT="${ROOT}/evidence/current"
EVIDENCE_DIR="${EVIDENCE_ROOT}/logs"
rm -rf "${EVIDENCE_DIR}"
mkdir -p "${EVIDENCE_DIR}"

# Official SDK feature checks pull alloy/aws-lc/rustls/icu. Keep low-resource defaults stable.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export RUSTFLAGS="${RUSTFLAGS:--C debuginfo=0}"

cd "${ROOT}"

cargo fmt --check 2>&1 | tee "${EVIDENCE_DIR}/01-cargo-fmt.log"
cargo check --workspace --locked 2>&1 | tee "${EVIDENCE_DIR}/02-cargo-check.log"
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings 2>&1 | tee "${EVIDENCE_DIR}/03-cargo-clippy.log"

# Keep deterministic workspace tests separate from environment-gated PostgreSQL HTTP tests.
# If PMX_TEST_DATABASE_URL is exported, a plain `cargo test --workspace` would also run
# `pmx-api`'s PostgreSQL E2E test and can make the generic workspace gate depend on local
# database lifecycle. The API fake E2E and PostgreSQL E2E are run explicitly below.
cargo test --workspace --exclude pmx-api --locked -- --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/04-cargo-test-workspace-non-api.log"
cargo test -p pmx-api --test http_and_fake_e2e --locked -- --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/05-http-fake-e2e.log"

cargo test --manifest-path adapters/pmx-official-sdk-spike/Cargo.toml --locked 2>&1 | tee "${EVIDENCE_DIR}/06-sdk-spike-no-features.log"
cargo test --manifest-path adapters/pmx-official-sdk-spike/Cargo.toml --features sdk-typecheck --locked 2>&1 | tee "${EVIDENCE_DIR}/07-sdk-spike-typecheck.log"

cargo fmt --check --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml 2>&1 | tee "${EVIDENCE_DIR}/08-sdk-adapter-fmt.log"
cargo check --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --locked 2>&1 | tee "${EVIDENCE_DIR}/09-sdk-adapter-check.log"
cargo clippy --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --all-targets --all-features --locked -- -D warnings 2>&1 | tee "${EVIDENCE_DIR}/10-sdk-adapter-clippy.log"
cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --locked 2>&1 | tee "${EVIDENCE_DIR}/11-sdk-adapter-test.log"
cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features sdk-typecheck --locked 2>&1 | tee "${EVIDENCE_DIR}/12-sdk-adapter-typecheck.log"

if [[ -n "${PMX_TEST_DATABASE_URL:-}" ]]; then
  psql "${PMX_TEST_DATABASE_URL}" -f migrations/0001_initial.sql 2>&1 | tee "${EVIDENCE_DIR}/13-pg-migration.log"
  cargo test -p pmx-store postgres::tests --locked -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/14-pg-store-tests.log"
  cargo test -p pmx-api --test http_postgres_e2e --locked -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/15-http-postgres-e2e.log"
else
  echo "PMX_TEST_DATABASE_URL not set; PostgreSQL repository/API proof skipped" | tee "${EVIDENCE_DIR}/13-pg-skipped.log"
fi

if [[ "${PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE:-}" == "1" ]]; then
  cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features authenticated-smoke --locked authenticated_non_trading_smoke -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/16-authenticated-smoke.log"
fi

if [[ "${PMX_RUN_SIGN_ONLY_DRY_RUN:-}" == "1" ]]; then
  cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features sign-only-dry-run --locked sign_only_dry_run -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/17-sign-only-dry-run.log"
fi

# Release hygiene should be evaluated on a clean release snapshot, not on a dirty developer
# working tree with .env, target/, temporary PostgreSQL data, or evidence logs.
SNAPSHOT_DIR="$(mktemp -d)"
cleanup() { rm -rf "${SNAPSHOT_DIR}"; }
trap cleanup EXIT

if git -C "${ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git -C "${ROOT}" archive --format=tar HEAD | tar -x -C "${SNAPSHOT_DIR}"
  HYGIENE_ROOT="${SNAPSHOT_DIR}"
else
  tar -C "${ROOT}" \
    --exclude='./.git' \
    --exclude='./.env' \
    --exclude='./target' \
    --exclude='./evidence' \
    --exclude='./adapters/pmx-official-sdk-adapter/target' \
    --exclude='./adapters/pmx-official-sdk-spike/target' \
    -cf - . | tar -x -C "${SNAPSHOT_DIR}"
  HYGIENE_ROOT="${SNAPSHOT_DIR}"
fi
python validation/check_plan_storage.py 2>&1 | tee "${EVIDENCE_DIR}/18-plan-storage-guard.log"
python validation/check_live_submit_guard.py 2>&1 | tee "${EVIDENCE_DIR}/19-live-submit-static-guard.log"
python validation/check_sign_only_lifecycle.py 2>&1 | tee "${EVIDENCE_DIR}/20-sign-only-lifecycle-guard.log"
python validation/check_runtime_worker_models.py 2>&1 | tee "${EVIDENCE_DIR}/21-runtime-worker-model-guard.log"
python validation/check_v0_23_evidence_manifest.py 2>&1 | tee "${EVIDENCE_DIR}/23-v0-23-evidence-manifest-guard.log"
python scripts/check_release_hygiene.py "${HYGIENE_ROOT}" 2>&1 | tee "${EVIDENCE_DIR}/26-release-hygiene-clean-snapshot.log"
python validation/write_v0_23_evidence_manifest.py "${EVIDENCE_DIR}" >/dev/null

if [[ -f "${INTEGRATION_ROOT}/scripts/check_version_consistency.py" && -f "${INTEGRATION_ROOT}/scripts/validate_contracts.py" ]]; then
  python validation/check_v0_23_lifecycle_api.py 2>&1 | tee "${EVIDENCE_DIR}/22-v0-23-lifecycle-api-guard.log"
  python "${INTEGRATION_ROOT}/scripts/check_version_consistency.py" 2>&1 | tee "${EVIDENCE_DIR}/24-version-consistency-guard.log"
  python "${INTEGRATION_ROOT}/scripts/validate_contracts.py" 2>&1 | tee "${EVIDENCE_DIR}/25-contract-validation.log"
  ARTIFACT_PATH="$(python "${INTEGRATION_ROOT}/scripts/package_release.py" | tee "${EVIDENCE_DIR}/27-package-release.log" | tail -n 1)"
  python "${INTEGRATION_ROOT}/scripts/check_release_artifact.py" "${ARTIFACT_PATH}" "$(cat "${INTEGRATION_ROOT}/VERSION")" 2>&1 | tee "${EVIDENCE_DIR}/28-release-artifact-check.log"
  python validation/write_v0_23_evidence_manifest.py "${EVIDENCE_DIR}" "${ARTIFACT_PATH}" 2>&1 | tee "${EVIDENCE_DIR}/29-write-evidence-manifest.log"
  python validation/check_docs_evidence_governance.py 2>&1 | tee "${EVIDENCE_DIR}/30-docs-evidence-governance.log"
  python validation/write_v0_23_evidence_manifest.py "${EVIDENCE_DIR}" "${ARTIFACT_PATH}" 2>&1 | tee -a "${EVIDENCE_DIR}/29-write-evidence-manifest.log"
else
  echo "integration repository not found; lifecycle parity, contract validation, and release packaging skipped" | tee "${EVIDENCE_DIR}/22-integration-skipped.log"
  python validation/write_v0_23_evidence_manifest.py "${EVIDENCE_DIR}" 2>&1 | tee "${EVIDENCE_DIR}/29-write-evidence-manifest.log"
fi

echo "v0.23 gates completed; evidence in ${EVIDENCE_ROOT}"
