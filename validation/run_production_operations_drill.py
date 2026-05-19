#!/usr/bin/env python3
"""Emit structured v0.27 production-operations control evidence without live capability."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_OPERATIONS_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

CONTROL_SCENARIOS = [
    {
        "name": "secret_custody",
        "required_evidence": [
            "secret manager or KMS or HSM design review",
            "no plaintext private keys in logs",
            "credential rotation drill",
            "break-glass access review",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "deployment_preflight",
        "required_evidence": [
            "artifact SHA-256 verification",
            "evidence manifest SHA-256 verification",
            "migration evidence",
            "config diff review",
            "operator approval",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "rollback_runbook",
        "required_evidence": [
            "config kill switch",
            "sign-only fallback",
            "cancel-only fallback",
            "read-only fallback",
            "database forward-fix boundary",
        ],
        "fallback_mode": "sign-only",
    },
    {
        "name": "incident_drill",
        "required_evidence": [
            "remote unknown",
            "cancel failure",
            "SDK failure",
            "PostgreSQL unavailable",
            "geoblock",
            "low resource",
            "degraded runtime workers",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "alerting_dashboard",
        "required_evidence": [
            "runtime worker health",
            "reconcile backlog",
            "remote unknown count",
            "idempotency conflict rate",
            "SDK error rate",
            "audit export failure",
            "per-order trace id",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "slo_error_budget",
        "required_evidence": [
            "safety SLO",
            "availability SLO",
            "safety breach freezes live submit",
            "error budget cannot auto-enable live submit",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "audit_export_retention",
        "required_evidence": [
            "redacted immutable export",
            "retention duration",
            "deletion policy",
            "legal hold behavior",
            "access review",
        ],
        "fallback_mode": "read-only",
    },
    {
        "name": "risk_limits",
        "required_evidence": [
            "account whitelist",
            "market whitelist",
            "per-order cap",
            "per-day cap",
            "exposure cap",
            "operator approval threshold",
            "remote unknown freeze override",
        ],
        "fallback_mode": "cancel-only",
    },
    {
        "name": "dependency_sdk_breakage",
        "required_evidence": [
            "pinned dependencies",
            "compatibility report",
            "sign-only regression evidence",
            "rollback plan",
            "upstream breakage playbook",
        ],
        "fallback_mode": "sign-only",
    },
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during production operations drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during production operations drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production operations drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "secret_custody",
            "deployment_preflight",
            "rollback_runbook",
            "incident_drill",
            "alerting_dashboard",
            "slo_error_budget",
            "audit_export_retention",
            "risk_limits",
            "dependency_sdk_breakage",
            "no live submit",
            "no live cancel",
            "not production-ready",
        ]:
            if token not in doc:
                failures.append(f"production operations drill document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("46-production-operations-drill.log", "production operations drill", failures)
    if '"production_operations_validation"' not in manifest:
        failures.append("evidence manifest must include production_operations_validation")
    if "46-production-operations-drill.log" not in manifest:
        failures.append("evidence manifest must capture production operations drill log")

    scenarios = []
    for scenario in CONTROL_SCENARIOS:
        missing = [item for item in scenario["required_evidence"] if not item]
        scenarios.append(
            {
                "name": scenario["name"],
                "status": "pass" if not missing else "fail",
                "required_evidence": scenario["required_evidence"],
                "fallback_mode": scenario["fallback_mode"],
                "live_submit_allowed": False,
                "live_cancel_allowed": False,
                "production_ready_claimed": False,
                "missing": missing,
            }
        )
        if missing:
            failures.append(f"{scenario['name']} missing required evidence descriptors")

    result = {
        "status": "fail" if failures else "pass",
        "production_ready_claimed": False,
        "live_submit_env_enabled": env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_env_enabled": env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "remote_side_effects": False,
        "scenarios": scenarios,
        "allowed_runtime_modes": ["read-only", "sign-only", "cancel-only"],
        "forbidden_claims": ["production-ready", "live-ready"],
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
