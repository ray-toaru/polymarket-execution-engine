#!/usr/bin/env python3
"""Validate local production incident response matrix without side effects."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_INCIDENT_RESPONSE_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

INCIDENTS = [
    ("remote_unknown", "cancel-only"),
    ("cancel_failure", "operator-required"),
    ("sdk_failure", "sign-only"),
    ("postgres_unavailable", "read-only"),
    ("geoblock", "read-only"),
    ("low_resource", "read-only"),
    ("worker_degraded", "read-only"),
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during incident response drill")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production incident response drill document missing")
    for token in [name for name, _ in INCIDENTS] + [
        "live_submit_allowed = false",
        "remote_side_effects = false",
        "operator_required = true",
        "evidence_preserved = true",
        "production_ready_claimed = false",
    ]:
        if token not in doc:
            failures.append(f"production incident response document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("53-production-incident-response-drill.log", "production incident response drill", failures)
    if '"production_incident_response_validation"' not in manifest:
        failures.append("evidence manifest must include production_incident_response_validation")

    scenarios = [
        {
            "name": name,
            "fallback_mode": fallback,
            "live_submit_allowed": False,
            "live_cancel_allowed": False,
            "operator_required": True,
            "evidence_preserved": True,
            "remote_side_effects": False,
        }
        for name, fallback in INCIDENTS
    ]
    result = {
        "status": "fail" if failures else "pass",
        "scenarios": scenarios,
        "production_ready_claimed": False,
        "remote_side_effects": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
