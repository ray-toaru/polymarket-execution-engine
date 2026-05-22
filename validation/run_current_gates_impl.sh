#!/usr/bin/env bash
set -euo pipefail

# Current release entrypoint: validation/run_current_gates.sh delegates here.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INTEGRATION_ROOT="${PMX_INTEGRATION_ROOT:-$(cd "${ROOT}/.." && pwd)}"
EVIDENCE_ROOT="${ROOT}/evidence/current"
EVIDENCE_DIR="${EVIDENCE_ROOT}/logs"
rm -rf "${EVIDENCE_DIR}"
mkdir -p "${EVIDENCE_DIR}"

if [[ -f "${INTEGRATION_ROOT}/scripts/collect_validation_environment.py" ]]; then
  python "${INTEGRATION_ROOT}/scripts/collect_validation_environment.py" > "${EVIDENCE_ROOT}/environment.json"
fi

# Official SDK feature checks pull alloy/aws-lc/rustls/icu. Keep low-resource defaults stable.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export RUSTFLAGS="${RUSTFLAGS:--C debuginfo=0}"
# Credentialed SDK calls depend on remote CLOB metadata/auth responses and need a
# slightly wider timeout than local/unit/static gates to avoid false negatives.
export PMX_SDK_CALL_TIMEOUT_SECS="${PMX_SDK_CALL_TIMEOUT_SECS:-30}"

cd "${ROOT}"

run_with_empty_ok() {
  local output_path="$1"
  local empty_message="$2"
  shift 2
  "$@" 2>&1 | tee "${output_path}"
  if [[ ! -s "${output_path}" ]]; then
    printf 'passed: %s\n' "${empty_message}" > "${output_path}"
  fi
}

apply_all_pg_migrations() {
  for migration in migrations/[0-9]*.sql; do
    printf 'applying %s\n' "${migration}"
    psql "${PMX_TEST_DATABASE_URL}" -f "${migration}"
  done
}

run_with_empty_ok "${EVIDENCE_DIR}/01-cargo-fmt.log" "cargo fmt --check produced no output" cargo fmt --check
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

run_with_empty_ok "${EVIDENCE_DIR}/08-sdk-adapter-fmt.log" "SDK adapter cargo fmt --check produced no output" cargo fmt --check --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml
cargo check --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --locked 2>&1 | tee "${EVIDENCE_DIR}/09-sdk-adapter-check.log"
cargo clippy --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --all-targets --all-features --locked -- -D warnings 2>&1 | tee "${EVIDENCE_DIR}/10-sdk-adapter-clippy.log"
cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --locked 2>&1 | tee "${EVIDENCE_DIR}/11-sdk-adapter-test.log"
cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features sdk-typecheck --locked 2>&1 | tee "${EVIDENCE_DIR}/12-sdk-adapter-typecheck.log"

if [[ -n "${PMX_TEST_DATABASE_URL:-}" ]]; then
  apply_all_pg_migrations 2>&1 | tee "${EVIDENCE_DIR}/13-pg-migration.log"
  cargo test -p pmx-store postgres::postgres_tests --locked -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/14-pg-store-tests.log"
  cargo test -p pmx-api --test http_postgres_e2e --locked -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/15-http-postgres-e2e.log"
else
  echo "PMX_TEST_DATABASE_URL not set; PostgreSQL repository/API proof skipped" | tee "${EVIDENCE_DIR}/13-pg-skipped.log"
fi

if [[ "${PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE:-}" == "1" ]]; then
  cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features authenticated-smoke --locked authenticated_non_trading_smoke -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/16-authenticated-smoke.log"
else
  echo "PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE not set to 1; credentialed non-trading smoke skipped" | tee "${EVIDENCE_DIR}/16-authenticated-smoke-skipped.log"
fi

if [[ "${PMX_RUN_SIGN_ONLY_DRY_RUN:-}" == "1" ]]; then
  cargo test --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml --features sign-only-dry-run --locked sign_only_dry_run -- --nocapture --test-threads=1 2>&1 | tee "${EVIDENCE_DIR}/17-sign-only-dry-run.log"
else
  echo "PMX_RUN_SIGN_ONLY_DRY_RUN not set to 1; sign-only dry-run skipped" | tee "${EVIDENCE_DIR}/17-sign-only-dry-run-skipped.log"
fi

export PMX_RUN_SHADOW_EXECUTION_DRILL="${PMX_RUN_SHADOW_EXECUTION_DRILL:-1}"
python validation/run_shadow_execution_drill.py 2>&1 | tee "${EVIDENCE_DIR}/29-shadow-execution-drill.log"
python validation/run_reconciliation_drift_drill.py 2>&1 | tee "${EVIDENCE_DIR}/31-reconciliation-drift-drill.log"
python validation/run_kill_switch_rollback_drill.py 2>&1 | tee "${EVIDENCE_DIR}/32-kill-switch-rollback-drill.log"
python validation/check_shadow_rollback_drills.py 2>&1 | tee "${EVIDENCE_DIR}/44-shadow-rollback-drill-guard.log"

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
python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" >/dev/null
python validation/check_current_evidence_manifest.py 2>&1 | tee "${EVIDENCE_DIR}/23-current-evidence-manifest-guard.log"
python validation/check_migration_framework.py 2>&1 | tee "${EVIDENCE_DIR}/33-migration-framework-guard.log"
python validation/run_migration_drift_dry_run.py 2>&1 | tee "${EVIDENCE_DIR}/34-migration-drift-dry-run.log"
python validation/check_sdk_standard_sign_only.py 2>&1 | tee "${EVIDENCE_DIR}/35-sdk-standard-sign-only-guard.log"
python validation/check_production_readiness_guard.py 2>&1 | tee "${EVIDENCE_DIR}/36-production-readiness-guard.log"
python validation/check_sdk_regression_suite.py 2>&1 | tee "${EVIDENCE_DIR}/37-sdk-regression-suite-guard.log"
python validation/run_live_canary_readiness_drill.py 2>&1 | tee "${EVIDENCE_DIR}/38-live-canary-readiness-drill.log"
python validation/run_live_canary_blocked_drill.py 2>&1 | tee "${EVIDENCE_DIR}/39-live-canary-blocked-drill.log"
python validation/run_live_canary_rehearsal_drill.py 2>&1 | tee "${EVIDENCE_DIR}/40-live-canary-rehearsal-drill.log"
python validation/run_live_canary_preflight_drill.py 2>&1 | tee "${EVIDENCE_DIR}/45-live-canary-preflight-drill.log"
python validation/check_production_hardening_config.py 2>&1 | tee "${EVIDENCE_DIR}/41-production-hardening-config.log"
python validation/run_production_operations_drill.py 2>&1 | tee "${EVIDENCE_DIR}/46-production-operations-drill.log"
python validation/run_production_authorization_block_drill.py 2>&1 | tee "${EVIDENCE_DIR}/47-production-authorization-block-drill.log"
python validation/run_production_audit_export_drill.py 2>&1 | tee "${EVIDENCE_DIR}/48-production-audit-export-drill.log"
python validation/run_production_dependency_breakage_drill.py 2>&1 | tee "${EVIDENCE_DIR}/49-production-dependency-breakage-drill.log"
python validation/check_runtime_worker_status_query.py 2>&1 | tee "${EVIDENCE_DIR}/42-runtime-worker-status-query.log"
python validation/check_observability_evidence.py 2>&1 | tee "${EVIDENCE_DIR}/43-observability-evidence.log"
python scripts/check_release_hygiene.py "${HYGIENE_ROOT}" 2>&1 | tee "${EVIDENCE_DIR}/26-release-hygiene-clean-snapshot.log"
python validation/run_production_preflight_config_diff_review.py 2>&1 | tee "${EVIDENCE_DIR}/64-production-preflight-config-diff-review.log"
python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" >/dev/null

if [[ -f "${INTEGRATION_ROOT}/scripts/check_version_consistency.py" && -f "${INTEGRATION_ROOT}/scripts/validate_contracts.py" ]]; then
  python validation/check_current_lifecycle_api.py 2>&1 | tee "${EVIDENCE_DIR}/22-current-lifecycle-api-guard.log"
  python "${INTEGRATION_ROOT}/scripts/check_version_consistency.py" 2>&1 | tee "${EVIDENCE_DIR}/24-version-consistency-guard.log"
  python "${INTEGRATION_ROOT}/scripts/validate_contracts.py" 2>&1 | tee "${EVIDENCE_DIR}/25-contract-validation.log"
  ARTIFACT_PATH="$(python "${INTEGRATION_ROOT}/scripts/package_release.py" | tee "${EVIDENCE_DIR}/27-package-release.log" | tail -n 1)"
  python "${INTEGRATION_ROOT}/scripts/check_release_artifact.py" "${ARTIFACT_PATH}" "$(cat "${INTEGRATION_ROOT}/VERSION")" 2>&1 | tee "${EVIDENCE_DIR}/28-release-artifact-check.log"
  PMX_RELEASE_ARTIFACT_PATH="${ARTIFACT_PATH}" python validation/run_production_deployment_preflight_drill.py 2>&1 | tee "${EVIDENCE_DIR}/50-production-deployment-preflight-drill.log"
  PMX_RELEASE_ARTIFACT_PATH="${ARTIFACT_PATH}" python validation/run_production_secret_custody_drill.py 2>&1 | tee "${EVIDENCE_DIR}/51-production-secret-custody-drill.log"
  python validation/run_production_monitoring_slo_drill.py 2>&1 | tee "${EVIDENCE_DIR}/52-production-monitoring-slo-drill.log"
  python validation/run_production_incident_response_drill.py 2>&1 | tee "${EVIDENCE_DIR}/53-production-incident-response-drill.log"
  python validation/run_production_rollback_downgrade_drill.py 2>&1 | tee "${EVIDENCE_DIR}/54-production-rollback-downgrade-drill.log"
  python validation/run_production_risk_limits_drill.py 2>&1 | tee "${EVIDENCE_DIR}/55-production-risk-limits-drill.log"
  python validation/run_production_config_profile_drill.py 2>&1 | tee "${EVIDENCE_DIR}/56-production-config-profile-drill.log"
  python validation/run_production_release_decision_guard.py 2>&1 | tee "${EVIDENCE_DIR}/57-production-release-decision-guard.log"
  python validation/run_live_canary_controlled_prep_drill.py 2>&1 | tee "${EVIDENCE_DIR}/58-live-canary-controlled-prep-drill.log"
  python validation/run_external_secret_provider_preflight.py 2>&1 | tee "${EVIDENCE_DIR}/59-external-secret-provider-preflight.log"
  python validation/run_external_operator_approval_preflight.py 2>&1 | tee "${EVIDENCE_DIR}/60-external-operator-approval-preflight.log"
  python validation/run_external_alert_routing_preflight.py 2>&1 | tee "${EVIDENCE_DIR}/61-external-alert-routing-preflight.log"
python validation/run_production_preflight_config_guard.py 2>&1 | tee "${EVIDENCE_DIR}/62-production-preflight-config-guard.log"
python validation/run_production_preflight_config_fixture_drill.py 2>&1 | tee "${EVIDENCE_DIR}/63-production-preflight-config-fixture-drill.log"
python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" "${ARTIFACT_PATH}" >/dev/null
python validation/run_real_funds_canary_preflight_drill.py 2>&1 | tee "${EVIDENCE_DIR}/65-real-funds-canary-preflight.log"
python validation/run_real_funds_canary_lifecycle_drill.py 2>&1 | tee "${EVIDENCE_DIR}/66-real-funds-canary-lifecycle-drill.log"
python validation/run_real_funds_canary_ready_drill.py 2>&1 | tee "${EVIDENCE_DIR}/67-real-funds-canary-ready-drill.log"
python validation/run_real_funds_canary_review_package_drill.py 2>&1 | tee "${EVIDENCE_DIR}/68-real-funds-canary-review-package.log"
python validation/run_single_host_deployment_drill.py 2>&1 | tee "${EVIDENCE_DIR}/69-single-host-deployment-drill.log"
python validation/run_single_host_canary_candidate_drill.py 2>&1 | tee "${EVIDENCE_DIR}/70-single-host-canary-candidate-drill.log"
python validation/run_single_host_go_candidate_drill.py 2>&1 | tee "${EVIDENCE_DIR}/71-single-host-go-candidate-drill.log"
python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" "${ARTIFACT_PATH}" >/dev/null
python validation/check_docs_evidence_governance.py 2>&1 | tee "${EVIDENCE_DIR}/30-docs-evidence-governance.log"
python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" "${ARTIFACT_PATH}" >/dev/null
else
  echo "integration repository not found; lifecycle parity, contract validation, and release packaging skipped" | tee "${EVIDENCE_DIR}/22-integration-skipped.log"
  python validation/write_current_evidence_manifest.py "${EVIDENCE_DIR}" >/dev/null
fi

echo "current gates completed; evidence in ${EVIDENCE_ROOT}"
