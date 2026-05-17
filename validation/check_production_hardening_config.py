#!/usr/bin/env python3
"""Executable production-hardening config check without claiming production readiness."""
from __future__ import annotations

import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HARDENING_SPEC = ROOT / "docs" / "PRODUCTION_HARDENING_SPEC.md"
CONTROLS_MATRIX = ROOT / "docs" / "PRODUCTION_CONTROLS_MATRIX.md"
EVIDENCE_CONTROLS = ROOT / "docs" / "PRODUCTION_EVIDENCE_CONTROLS.md"
GATES = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST = ROOT / "validation" / "write_v0_23_evidence_manifest.py"

REQUIRED_SPEC_TOKENS = [
    "Secret Custody",
    "Deployment And Rollback",
    "Observability",
    "SLO And Error Budget",
    "Audit Export And Retention",
    "Risk Limits",
    "Dependency And SDK Breakage",
]

REQUIRED_CONTROL_TOKENS = [
    "Secret manager / KMS / HSM",
    "Production config profile",
    "Deployment runbook",
    "Rollback runbook",
    "Incident drill",
    "Alerting and dashboard",
    "SLO / error budget",
    "Audit export / retention policy",
]

REQUIRED_EVIDENCE_CONTROL_TOKENS = [
    "Exact artifact binding",
    "Full gate replay",
    "Runtime safety proof",
    "Redaction proof",
    "Rollback proof",
    "Operations proof",
]


def main() -> int:
    failures: list[str] = []
    if os.environ.get("PMX_ALLOW_LIVE_SUBMIT", "").strip() == "1":
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden in production hardening config check")
    if os.environ.get("PMX_ALLOW_LIVE_CANCEL", "").strip() == "1":
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden in production hardening config check")

    hardening = HARDENING_SPEC.read_text()
    for token in REQUIRED_SPEC_TOKENS:
        if token not in hardening:
            failures.append(f"production hardening spec missing token: {token}")

    controls = CONTROLS_MATRIX.read_text()
    for token in REQUIRED_CONTROL_TOKENS:
        if token not in controls:
            failures.append(f"production controls matrix missing token: {token}")

    evidence_controls = EVIDENCE_CONTROLS.read_text()
    for token in REQUIRED_EVIDENCE_CONTROL_TOKENS:
        if token not in evidence_controls:
            failures.append(f"production evidence controls missing token: {token}")

    gates = GATES.read_text()
    manifest = MANIFEST.read_text()
    if "41-production-hardening-config.log" not in gates:
        failures.append("run_v0_23_gates.sh must emit production hardening config log")
    if '"production_hardening_config_validation"' not in manifest:
        failures.append("evidence manifest must include production_hardening_config_validation")
    if "41-production-hardening-config.log" not in manifest:
        failures.append("evidence manifest must capture production hardening config log")

    result = {
        "status": "fail" if failures else "pass",
        "production_ready_claimed": False,
        "live_submit_enabled": os.environ.get("PMX_ALLOW_LIVE_SUBMIT", "").strip() == "1",
        "live_cancel_enabled": os.environ.get("PMX_ALLOW_LIVE_CANCEL", "").strip() == "1",
        "checks": [
            "secret_custody",
            "deployment_and_rollback",
            "observability",
            "slo_and_error_budget",
            "audit_export_and_retention",
            "risk_limits",
            "dependency_and_sdk_breakage",
            "production_evidence_controls",
        ],
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
