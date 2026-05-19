#!/usr/bin/env python3
"""Validate the external alert routing and pager preflight contract."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "EXTERNAL_ALERT_ROUTING_PREFLIGHT.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

REFERENCE_ENV = {
    "alert_provider_reference_present": "PMX_ALERT_PROVIDER",
    "alert_route_reference_present": "PMX_ALERT_ROUTE_ID",
    "pager_escalation_policy_present": "PMX_PAGER_ESCALATION_POLICY_ID",
    "dashboard_reference_present": "PMX_DASHBOARD_REFERENCE",
    "alert_test_evidence_present": "PMX_ALERT_TEST_EVIDENCE_ID",
}
ALERT_SIGNALS = [
    "runtime_worker_health_alert",
    "reconcile_backlog_alert",
    "remote_unknown_alert",
    "sdk_error_rate_alert",
    "audit_export_failure_alert",
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def present(name: str) -> bool:
    return bool(os.environ.get(name, "").strip())


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during external alert routing preflight")

    required_tokens = [
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
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("external alert routing preflight document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"external alert routing preflight document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("61-external-alert-routing-preflight.log", "external alert routing preflight", failures)
    if '"external_alert_routing_preflight_validation"' not in manifest:
        failures.append("evidence manifest must include external_alert_routing_preflight_validation")
    if "61-external-alert-routing-preflight.log" not in manifest:
        failures.append("evidence manifest must capture external alert routing preflight log")

    references = {label: present(env_name) for label, env_name in REFERENCE_ENV.items()}
    alert_signal_presence = {signal: True for signal in ALERT_SIGNALS}
    alerting_ready = all(references.values()) and all(alert_signal_presence.values())
    result = {
        "status": "fail" if failures else "pass",
        "references": references,
        "alert_signals": alert_signal_presence,
        "pager_ack_required": True,
        "alerting_ready": alerting_ready,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
