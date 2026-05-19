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

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("production readiness guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
