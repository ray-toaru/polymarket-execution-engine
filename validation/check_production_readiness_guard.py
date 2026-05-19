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
MONITORING_SLO_DRILL = ROOT / "docs" / "PRODUCTION_MONITORING_SLO_DRILL.md"
INCIDENT_RESPONSE_DRILL = ROOT / "docs" / "PRODUCTION_INCIDENT_RESPONSE_DRILL.md"
ROLLBACK_DOWNGRADE_DRILL = ROOT / "docs" / "PRODUCTION_ROLLBACK_DOWNGRADE_DRILL.md"
RISK_LIMITS_DRILL = ROOT / "docs" / "PRODUCTION_RISK_LIMITS_DRILL.md"
CONFIG_PROFILE_DRILL = ROOT / "docs" / "PRODUCTION_CONFIG_PROFILE_DRILL.md"
RELEASE_DECISION_GUARD = ROOT / "docs" / "PRODUCTION_RELEASE_DECISION_GUARD.md"
CONTROLLED_CANARY_PREP_DRILL = ROOT / "docs" / "LIVE_CANARY_CONTROLLED_PREP_DRILL.md"
EXTERNAL_SECRET_PROVIDER_PREFLIGHT = ROOT / "docs" / "EXTERNAL_SECRET_PROVIDER_PREFLIGHT.md"
EXTERNAL_OPERATOR_APPROVAL_PREFLIGHT = ROOT / "docs" / "EXTERNAL_OPERATOR_APPROVAL_PREFLIGHT.md"
EXTERNAL_ALERT_ROUTING_PREFLIGHT = ROOT / "docs" / "EXTERNAL_ALERT_ROUTING_PREFLIGHT.md"
PRODUCTION_PREFLIGHT_CONFIG_GUARD = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_GUARD.md"
PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_DRILL = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_DRILL.md"
PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW.md"
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

MONITORING_SLO_TOKENS = [
    "runtime_worker_health",
    "reconcile_backlog",
    "remote_unknown_count",
    "idempotency_conflict_rate",
    "sdk_error_rate",
    "audit_export_failure",
    "stale_worker_heartbeat",
    "geoblock_blocked",
    "postgres_unavailable",
    "safety_slo_breach_freezes_live_submit = true",
    "availability_recovery_auto_enables_live_submit = false",
    "error_budget_auto_enables_live_submit = false",
    "remote_side_effects = false",
]

INCIDENT_RESPONSE_TOKENS = [
    "remote_unknown",
    "cancel_failure",
    "sdk_failure",
    "postgres_unavailable",
    "geoblock",
    "low_resource",
    "worker_degraded",
    "live_submit_allowed = false",
    "operator_required = true",
    "evidence_preserved = true",
    "remote_side_effects = false",
]

ROLLBACK_DOWNGRADE_TOKENS = [
    "sdk_failure_to_sign_only",
    "remote_unknown_to_cancel_only",
    "postgres_unavailable_to_read_only",
    "geoblock_to_read_only",
    "kill_switch_to_read_only",
    "recovery_requires_operator_review",
    "sign-only",
    "cancel-only",
    "read-only",
    "auto_reenable_live_submit = false",
    "remote_side_effects = false",
]

RISK_LIMITS_TOKENS = [
    "account_whitelist",
    "market_whitelist",
    "per_order_cap",
    "per_day_cap",
    "exposure_cap",
    "operator_approval_threshold",
    "remote_unknown_freeze_override",
    "stale_market_data_blocks",
    "geoblock_blocks",
    "live_submit_allowed = false",
]

CONFIG_PROFILE_TOKENS = [
    "live_submit_default_disabled",
    "live_cancel_default_disabled",
    "production_ready_default_false",
    "kill_switch_default_closed",
    "per_account_enablement_required",
    "per_market_enablement_required",
    "amount_caps_required",
    "operator_approval_required",
    "canary_profile_isolated",
    "live_submit_allowed = false",
]

RELEASE_DECISION_TOKENS = [
    "release_status_not_production_ready",
    "release_status_not_live_ready",
    "validated_release_false",
    "production_ready_false",
    "live_trading_ready_false",
    "production_blocker_present",
    "live_blocker_present",
    "artifact_kind_source_candidate",
    "no_production_promotion_without_review",
    "production_ready_claimed = false",
]

CONTROLLED_CANARY_PREP_TOKENS = [
    "compile_feature_live_submit",
    "env_allow_live_submit",
    "config_allow_live_submit",
    "operator_approval_present",
    "account_whitelisted",
    "market_whitelisted",
    "tiny_size_cap",
    "limit_order_only",
    "idempotency_key_written",
    "repository_reservation_exists",
    "reconcile_after_submit_required",
    "remote_unknown_freezes_submit",
    "cancel_only_fallback_ready",
    "canary_submit_allowed = false",
    "remote_side_effects = false",
]

EXTERNAL_SECRET_PROVIDER_TOKENS = [
    "secret_provider_reference_present",
    "kms_key_reference_present",
    "rotation_evidence_reference_present",
    "break_glass_review_reference_present",
    "plaintext_secret_values_absent",
    "provider_health_check_required",
    "credential_rotation_required",
    "break_glass_review_required",
    "external_secret_custody_ready = false",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
]

EXTERNAL_OPERATOR_APPROVAL_TOKENS = [
    "approval_id_present",
    "approval_hash_present",
    "approval_ticket_present",
    "approver_identity_present",
    "approval_expiry_present",
    "approval_scope_present",
    "dual_control_required",
    "approval_replay_block_required",
    "approval_expiry_enforced",
    "operator_approval_ready = false",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
]

EXTERNAL_ALERT_ROUTING_TOKENS = [
    "alert_provider_reference_present",
    "alert_route_reference_present",
    "pager_escalation_policy_present",
    "dashboard_reference_present",
    "alert_test_evidence_present",
    "runtime_worker_health_alert",
    "reconcile_backlog_alert",
    "remote_unknown_alert",
    "sdk_error_rate_alert",
    "audit_export_failure_alert",
    "pager_ack_required",
    "alerting_ready = false",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
]

PRODUCTION_PREFLIGHT_CONFIG_TOKENS = [
    "production_preflight_config_schema_version = 1",
    "secret_provider_reference_present",
    "kms_key_reference_present",
    "rotation_evidence_reference_present",
    "break_glass_review_reference_present",
    "approval_id_present",
    "approval_hash_present",
    "approval_ticket_present",
    "approver_identity_present",
    "approval_expiry_present",
    "approval_scope_present",
    "alert_provider_reference_present",
    "alert_route_reference_present",
    "pager_escalation_policy_present",
    "dashboard_reference_present",
    "alert_test_evidence_present",
    "forbidden_sensitive_keys_absent = true",
    "forbidden_sensitive_values_absent = true",
    "references_only_no_secret_values = true",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
]

PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_TOKENS = [
    "fixture_secret_provider_ready = true",
    "fixture_operator_approval_ready = true",
    "fixture_alerting_ready = true",
    "fixture_live_submit_allowed = false",
    "fixture_live_cancel_allowed = false",
    "fixture_remote_side_effects = false",
    "invalid_sensitive_fixture_rejected = true",
    "invalid_sensitive_fixture_secret_value_logged = false",
    "invalid_sensitive_fixture_reports_path_only = true",
    "forbidden_sensitive_keys_absent = false",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
]

PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW_TOKENS = [
    "PMX_PRODUCTION_PREFLIGHT_BASELINE_CONFIG",
    "PMX_PRODUCTION_PREFLIGHT_CANDIDATE_CONFIG",
    "config_diff_review_passed = true",
    "config_diff_review_rejected_sensitive_candidate = true",
    "config_diff_review_secret_value_logged = false",
    "config_diff_review_reports_path_only = true",
    "config_diff_summary_uses_hashes = true",
    "changed_field_paths_present = true",
    "baseline_config_hash_present = true",
    "candidate_config_hash_present = true",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "remote_side_effects = false",
    "production_ready_claimed = false",
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

    monitoring_slo_drill = MONITORING_SLO_DRILL.read_text()
    for token in MONITORING_SLO_TOKENS:
        if token not in monitoring_slo_drill:
            failures.append(f"production monitoring SLO drill missing {token}")

    incident_response_drill = INCIDENT_RESPONSE_DRILL.read_text()
    for token in INCIDENT_RESPONSE_TOKENS:
        if token not in incident_response_drill:
            failures.append(f"production incident response drill missing {token}")

    rollback_downgrade_drill = ROLLBACK_DOWNGRADE_DRILL.read_text()
    for token in ROLLBACK_DOWNGRADE_TOKENS:
        if token not in rollback_downgrade_drill:
            failures.append(f"production rollback downgrade drill missing {token}")

    risk_limits_drill = RISK_LIMITS_DRILL.read_text()
    for token in RISK_LIMITS_TOKENS:
        if token not in risk_limits_drill:
            failures.append(f"production risk limits drill missing {token}")

    config_profile_drill = CONFIG_PROFILE_DRILL.read_text()
    for token in CONFIG_PROFILE_TOKENS:
        if token not in config_profile_drill:
            failures.append(f"production config profile drill missing {token}")

    release_decision_guard = RELEASE_DECISION_GUARD.read_text()
    for token in RELEASE_DECISION_TOKENS:
        if token not in release_decision_guard:
            failures.append(f"production release decision guard missing {token}")

    controlled_canary_prep_drill = CONTROLLED_CANARY_PREP_DRILL.read_text()
    for token in CONTROLLED_CANARY_PREP_TOKENS:
        if token not in controlled_canary_prep_drill:
            failures.append(f"controlled canary prep drill missing {token}")

    external_secret_provider_preflight = EXTERNAL_SECRET_PROVIDER_PREFLIGHT.read_text()
    for token in EXTERNAL_SECRET_PROVIDER_TOKENS:
        if token not in external_secret_provider_preflight:
            failures.append(f"external secret provider preflight missing {token}")

    external_operator_approval_preflight = EXTERNAL_OPERATOR_APPROVAL_PREFLIGHT.read_text()
    for token in EXTERNAL_OPERATOR_APPROVAL_TOKENS:
        if token not in external_operator_approval_preflight:
            failures.append(f"external operator approval preflight missing {token}")

    external_alert_routing_preflight = EXTERNAL_ALERT_ROUTING_PREFLIGHT.read_text()
    for token in EXTERNAL_ALERT_ROUTING_TOKENS:
        if token not in external_alert_routing_preflight:
            failures.append(f"external alert routing preflight missing {token}")

    production_preflight_config_guard = PRODUCTION_PREFLIGHT_CONFIG_GUARD.read_text()
    for token in PRODUCTION_PREFLIGHT_CONFIG_TOKENS:
        if token not in production_preflight_config_guard:
            failures.append(f"production preflight config guard missing {token}")

    production_preflight_config_fixture_drill = PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_DRILL.read_text()
    for token in PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_TOKENS:
        if token not in production_preflight_config_fixture_drill:
            failures.append(f"production preflight config fixture drill missing {token}")

    production_preflight_config_diff_review = PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW.read_text()
    for token in PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW_TOKENS:
        if token not in production_preflight_config_diff_review:
            failures.append(f"production preflight config diff review missing {token}")

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
    require_current_gate_log("52-production-monitoring-slo-drill.log", "production monitoring SLO drill", failures)
    require_current_gate_log("53-production-incident-response-drill.log", "production incident response drill", failures)
    require_current_gate_log("54-production-rollback-downgrade-drill.log", "production rollback downgrade drill", failures)
    require_current_gate_log("55-production-risk-limits-drill.log", "production risk limits drill", failures)
    require_current_gate_log("56-production-config-profile-drill.log", "production config profile drill", failures)
    require_current_gate_log("57-production-release-decision-guard.log", "production release decision guard", failures)
    require_current_gate_log("58-live-canary-controlled-prep-drill.log", "controlled live canary prep drill", failures)
    require_current_gate_log("59-external-secret-provider-preflight.log", "external secret provider preflight", failures)
    require_current_gate_log("60-external-operator-approval-preflight.log", "external operator approval preflight", failures)
    require_current_gate_log("61-external-alert-routing-preflight.log", "external alert routing preflight", failures)
    require_current_gate_log("62-production-preflight-config-guard.log", "production preflight config guard", failures)
    require_current_gate_log("63-production-preflight-config-fixture-drill.log", "production preflight config fixture drill", failures)
    require_current_gate_log("64-production-preflight-config-diff-review.log", "production preflight config diff review", failures)
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
    if '"production_monitoring_slo_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_monitoring_slo_validation")
    if "52-production-monitoring-slo-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production monitoring SLO drill log")
    if '"production_incident_response_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_incident_response_validation")
    if "53-production-incident-response-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production incident response drill log")
    if '"production_rollback_downgrade_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_rollback_downgrade_validation")
    if "54-production-rollback-downgrade-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production rollback downgrade drill log")
    if '"production_risk_limits_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_risk_limits_validation")
    if "55-production-risk-limits-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production risk limits drill log")
    if '"production_config_profile_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_config_profile_validation")
    if "56-production-config-profile-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production config profile drill log")
    if '"production_release_decision_guard_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_release_decision_guard_validation")
    if "57-production-release-decision-guard.log" not in manifest_writer:
        failures.append("evidence manifest must capture production release decision guard log")
    if '"live_canary_controlled_prep_validation"' not in manifest_writer:
        failures.append("evidence manifest must include live_canary_controlled_prep_validation")
    if "58-live-canary-controlled-prep-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture controlled live canary prep drill log")
    if '"external_secret_provider_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include external_secret_provider_preflight_validation")
    if "59-external-secret-provider-preflight.log" not in manifest_writer:
        failures.append("evidence manifest must capture external secret provider preflight log")
    if '"external_operator_approval_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include external_operator_approval_preflight_validation")
    if "60-external-operator-approval-preflight.log" not in manifest_writer:
        failures.append("evidence manifest must capture external operator approval preflight log")
    if '"external_alert_routing_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include external_alert_routing_preflight_validation")
    if "61-external-alert-routing-preflight.log" not in manifest_writer:
        failures.append("evidence manifest must capture external alert routing preflight log")
    if '"production_preflight_config_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_preflight_config_validation")
    if "62-production-preflight-config-guard.log" not in manifest_writer:
        failures.append("evidence manifest must capture production preflight config guard log")
    if '"production_preflight_config_fixture_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_preflight_config_fixture_validation")
    if "63-production-preflight-config-fixture-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production preflight config fixture drill log")
    if '"production_preflight_config_diff_review_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_preflight_config_diff_review_validation")
    if "64-production-preflight-config-diff-review.log" not in manifest_writer:
        failures.append("evidence manifest must capture production preflight config diff review log")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("production readiness guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
