#!/usr/bin/env python3
"""Guard productionization governance without claiming production readiness."""
from __future__ import annotations

import json
import sys
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
RUNBOOK = ROOT / "docs" / "PRODUCTIONIZATION_RUNBOOK.md"
CONTROLS_MATRIX = ROOT / "docs" / "PRODUCTION_CONTROLS_MATRIX.md"
HARDENING_SPEC = ROOT / "docs" / "PRODUCTION_HARDENING_SPEC.md"
EVIDENCE_CONTROLS = ROOT / "docs" / "PRODUCTION_EVIDENCE_CONTROLS.md"
OPERATIONS_DRILL = ROOT / "docs" / "PRODUCTION_OPERATIONS_DRILL.md"
AUTHORIZATION_BLOCK_DRILL = ROOT / "docs" / "PRODUCTION_AUTHORIZATION_BLOCK_DRILL.md"
AUDIT_EXPORT_DRILL = ROOT / "docs" / "PRODUCTION_AUDIT_EXPORT_DRILL.md"
DEPENDENCY_BREAKAGE_DRILL = ROOT / "docs" / "PRODUCTION_DEPENDENCY_BREAKAGE_DRILL.md"
DEPLOYMENT_PREFLIGHT_DRILL = ROOT / "docs" / "PRODUCTION_DEPLOYMENT_PREFLIGHT_DRILL.md"
SECRET_CUSTODY_DRILL = ROOT / "docs" / "PRODUCTION_SECRET_CUSTODY_DRILL.md"
RELEASE_MANIFEST = ROOT / "release" / "manifest.json"
EVIDENCE_GUARD = ROOT / "validation" / "check_current_evidence_manifest.py"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"

RUNBOOK_TOKENS = [
    "Secret manager",
    "KMS",
    "HSM",
    "Production config profile",
    "Deployment runbook",
    "Rollback runbook",
    "Incident drill",
    "Alerting and dashboard",
    "SLO and error budget",
    "Audit export and retention policy",
    "Account and market risk limits",
    "Dependency update policy",
    "SDK upstream breakage playbook",
    "production-ready is forbidden",
]

CONTROLS_TOKENS = [
    "Secret manager / KMS / HSM",
    "Production config profile",
    "Deployment runbook",
    "Rollback runbook",
    "Incident drill",
    "Alerting and dashboard",
    "SLO / error budget",
    "Audit export / retention policy",
    "Account risk limits",
    "Market risk limits",
    "Dependency update policy",
    "SDK upstream breakage playbook",
    "artifact",
    "non-production",
]

HARDENING_TOKENS = [
    "Secret Custody",
    "secret manager",
    "KMS",
    "HSM",
    "Deployment And Rollback",
    "artifact SHA-256",
    "config kill switch",
    "Observability",
    "runtime worker health",
    "remote unknown freeze",
    "SLO And Error Budget",
    "Audit Export And Retention",
    "Risk Limits",
    "Account whitelist",
    "Market whitelist",
    "Dependency And SDK Breakage",
    "sign-only regression evidence",
]

EVIDENCE_CONTROL_TOKENS = [
    "Exact artifact binding",
    "Full gate replay",
    "Credentialed non-trading proof",
    "Runtime safety proof",
    "Canary proof",
    "Redaction proof",
    "Rollback proof",
    "Operations proof",
    "exact artifact SHA-256",
    "Production promotion is forbidden",
]

OPERATIONS_DRILL_TOKENS = [
    "secret_custody",
    "deployment_preflight",
    "rollback_runbook",
    "incident_drill",
    "alerting_dashboard",
    "slo_error_budget",
    "audit_export_retention",
    "risk_limits",
    "dependency_sdk_breakage",
    "not production-ready",
]

AUTHORIZATION_BLOCK_TOKENS = [
    "compile_feature_live_submit",
    "env_allow_live_submit",
    "config_allow_live_submit",
    "kill_switch_open",
    "runtime_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_healthy",
    "account_whitelisted",
    "market_whitelisted",
    "per_order_cap_ok",
    "per_day_cap_ok",
    "operator_approval_present",
    "reviewed_release_decision_present",
    "remote_side_effects = false",
]

AUDIT_EXPORT_TOKENS = [
    "trace_id",
    "signed_order_ref",
    "signed_order_digest",
    "retention_policy_id",
    "export_batch_id",
    "private_key",
    "clob_secret",
    "raw_signed_payload",
    "raw_signature",
    "SignedOrderEnvelope",
    "immutable_export = true",
    "redacted_export = true",
    "remote_side_effects = false",
]

DEPENDENCY_BREAKAGE_TOKENS = [
    "exact_sdk_pin",
    "adapter_lockfile_present",
    "spike_lockfile_present",
    "sdk_typecheck_evidence",
    "sign_only_regression_evidence",
    "authenticated_non_trading_evidence",
    "rollback_plan",
    "compatibility_review_required",
    "freeze_live_submit",
    "downgrade_to_sign_only",
    "downgrade_to_read_only",
    "preserve_evidence",
    "remote_side_effects = false",
]

DEPLOYMENT_PREFLIGHT_TOKENS = [
    "artifact_sha256_verified",
    "artifact_sidecar_verified",
    "evidence_sidecar_verified",
    "evidence_manifest_sha256_bound",
    "migration_evidence_present",
    "config_diff_review_required",
    "operator_approval_required",
    "live_submit_disabled",
    "live_cancel_disabled",
    "deploy_allowed = false",
    "remote_side_effects = false",
]

SECRET_CUSTODY_TOKENS = [
    "sensitive_env_detected_as_boolean_only",
    "sensitive_env_values_absent_from_logs",
    "sensitive_env_values_absent_from_manifest",
    "env_file_absent_from_artifact",
    "artifact_contains_no_env_file",
    "package_excludes_env_file",
    "no_plaintext_private_keys_logged",
    "no_clob_secret_logged",
    "rotation_drill_required",
    "break_glass_review_required",
    "secret_values_logged = false",
    "artifact_contains_env_file = false",
    "remote_side_effects = false",
]


def main() -> int:
    failures: list[str] = []
    runbook = RUNBOOK.read_text()
    for token in RUNBOOK_TOKENS:
        if token not in runbook:
            failures.append(f"production runbook missing {token}")

    controls = CONTROLS_MATRIX.read_text()
    for token in CONTROLS_TOKENS:
        if token not in controls:
            failures.append(f"production controls matrix missing {token}")

    hardening = HARDENING_SPEC.read_text()
    for token in HARDENING_TOKENS:
        if token not in hardening:
            failures.append(f"production hardening spec missing {token}")

    evidence_controls = EVIDENCE_CONTROLS.read_text()
    for token in EVIDENCE_CONTROL_TOKENS:
        if token not in evidence_controls:
            failures.append(f"production evidence controls missing {token}")

    operations_drill = OPERATIONS_DRILL.read_text()
    for token in OPERATIONS_DRILL_TOKENS:
        if token not in operations_drill:
            failures.append(f"production operations drill missing {token}")

    authorization_block_drill = AUTHORIZATION_BLOCK_DRILL.read_text()
    for token in AUTHORIZATION_BLOCK_TOKENS:
        if token not in authorization_block_drill:
            failures.append(f"production authorization block drill missing {token}")

    audit_export_drill = AUDIT_EXPORT_DRILL.read_text()
    for token in AUDIT_EXPORT_TOKENS:
        if token not in audit_export_drill:
            failures.append(f"production audit export drill missing {token}")

    dependency_breakage_drill = DEPENDENCY_BREAKAGE_DRILL.read_text()
    for token in DEPENDENCY_BREAKAGE_TOKENS:
        if token not in dependency_breakage_drill:
            failures.append(f"production dependency breakage drill missing {token}")

    deployment_preflight_drill = DEPLOYMENT_PREFLIGHT_DRILL.read_text()
    for token in DEPLOYMENT_PREFLIGHT_TOKENS:
        if token not in deployment_preflight_drill:
            failures.append(f"production deployment preflight drill missing {token}")

    secret_custody_drill = SECRET_CUSTODY_DRILL.read_text()
    for token in SECRET_CUSTODY_TOKENS:
        if token not in secret_custody_drill:
            failures.append(f"production secret custody drill missing {token}")

    release = json.loads(RELEASE_MANIFEST.read_text())
    status = str(release.get("status", "")).lower()
    if "production-ready" in status or "production_ready" in status:
        failures.append("release manifest must not claim production-ready")
    if "production-readiness-not-claimed" not in release.get("remaining_blockers", []):
        failures.append("release manifest must preserve production-readiness-not-claimed blocker")

    evidence_guard = EVIDENCE_GUARD.read_text()
    for token in ["validated_release=true", "artifact.sha256", "non-pass evidence sections"]:
        if token not in evidence_guard:
            failures.append(f"evidence guard missing anti-overclaim token: {token}")

    manifest_writer = MANIFEST_WRITER.read_text()
    require_current_gate_log("36-production-readiness-guard.log", "production readiness guard", failures)
    require_current_gate_log("41-production-hardening-config.log", "production hardening config", failures)
    require_current_gate_log("46-production-operations-drill.log", "production operations drill", failures)
    require_current_gate_log("47-production-authorization-block-drill.log", "production authorization block drill", failures)
    require_current_gate_log("48-production-audit-export-drill.log", "production audit export drill", failures)
    require_current_gate_log("49-production-dependency-breakage-drill.log", "production dependency breakage drill", failures)
    require_current_gate_log("50-production-deployment-preflight-drill.log", "production deployment preflight drill", failures)
    require_current_gate_log("51-production-secret-custody-drill.log", "production secret custody drill", failures)
    if '"productionization_validation"' not in manifest_writer:
        failures.append("evidence manifest must include productionization_validation")
    if "36-production-readiness-guard.log" not in manifest_writer:
        failures.append("evidence manifest must capture production readiness guard log")
    if '"production_hardening_config_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_hardening_config_validation")
    if '"production_operations_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_operations_validation")
    if "46-production-operations-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production operations drill log")
    if '"production_authorization_block_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_authorization_block_validation")
    if "47-production-authorization-block-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production authorization block drill log")
    if '"production_audit_export_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_audit_export_validation")
    if "48-production-audit-export-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production audit export drill log")
    if '"production_dependency_breakage_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_dependency_breakage_validation")
    if "49-production-dependency-breakage-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production dependency breakage drill log")
    if '"production_deployment_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_deployment_preflight_validation")
    if "50-production-deployment-preflight-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production deployment preflight drill log")
    if '"production_secret_custody_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_secret_custody_validation")
    if "51-production-secret-custody-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production secret custody drill log")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("production readiness guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
