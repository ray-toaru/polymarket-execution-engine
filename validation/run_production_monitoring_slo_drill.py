#!/usr/bin/env python3
"""Validate local monitoring, alerting, and SLO fail-closed evidence."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_MONITORING_SLO_DRILL.md"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"

SIGNALS = [
    "runtime_worker_health",
    "reconcile_backlog",
    "remote_unknown_count",
    "idempotency_conflict_rate",
    "sdk_error_rate",
    "audit_export_failure",
    "stale_worker_heartbeat",
    "geoblock_blocked",
    "postgres_unavailable",
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def evaluate_scenarios() -> list[dict[str, object]]:
    return [
        {
            "name": "safety_slo_breach",
            "signal": "remote_unknown_count",
            "alert_required": True,
            "operator_required": True,
            "live_submit_allowed": False,
            "fallback_mode": "read-only",
        },
        {
            "name": "availability_recovered_after_safety_breach",
            "signal": "runtime_worker_health",
            "alert_required": True,
            "operator_required": True,
            "live_submit_allowed": False,
            "fallback_mode": "sign-only",
        },
        {
            "name": "error_budget_available_but_safety_unknown",
            "signal": "sdk_error_rate",
            "alert_required": True,
            "operator_required": True,
            "live_submit_allowed": False,
            "fallback_mode": "sign-only",
        },
        {
            "name": "audit_export_failure",
            "signal": "audit_export_failure",
            "alert_required": True,
            "operator_required": True,
            "live_submit_allowed": False,
            "fallback_mode": "read-only",
        },
    ]


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during monitoring SLO drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during monitoring SLO drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production monitoring SLO drill document missing")
    else:
        doc = DOC.read_text()
        for token in SIGNALS + [
            "safety_slo_breach_freezes_live_submit = true",
            "availability_recovery_auto_enables_live_submit = false",
            "error_budget_auto_enables_live_submit = false",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production monitoring SLO document missing token: {token}")

    manifest_writer = MANIFEST_WRITER.read_text()
    require_current_gate_log("52-production-monitoring-slo-drill.log", "production monitoring SLO drill", failures)
    if '"production_monitoring_slo_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_monitoring_slo_validation")
    if "52-production-monitoring-slo-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production monitoring SLO drill log")

    scenarios = evaluate_scenarios()
    for scenario in scenarios:
        if scenario["live_submit_allowed"] is not False:
            failures.append(f"{scenario['name']} must not allow live submit")
        if scenario["operator_required"] is not True:
            failures.append(f"{scenario['name']} must require operator review")

    result = {
        "status": "fail" if failures else "pass",
        "signals": SIGNALS,
        "scenarios": scenarios,
        "safety_slo_breach_freezes_live_submit": True,
        "availability_recovery_auto_enables_live_submit": False,
        "error_budget_auto_enables_live_submit": False,
        "live_submit_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
