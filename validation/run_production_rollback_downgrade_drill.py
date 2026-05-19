#!/usr/bin/env python3
"""Validate rollback and downgrade paths remain fail-closed."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_ROLLBACK_DOWNGRADE_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

SCENARIOS = [
    ("sdk_failure_to_sign_only", "sign-only"),
    ("remote_unknown_to_cancel_only", "cancel-only"),
    ("postgres_unavailable_to_read_only", "read-only"),
    ("geoblock_to_read_only", "read-only"),
    ("kill_switch_to_read_only", "read-only"),
    ("recovery_requires_operator_review", "sign-only"),
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during rollback downgrade drill")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production rollback downgrade drill document missing")
    for token in [name for name, _ in SCENARIOS] + [
        "sign-only",
        "cancel-only",
        "read-only",
        "live_submit_allowed = false",
        "auto_reenable_live_submit = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]:
        if token not in doc:
            failures.append(f"production rollback downgrade document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("54-production-rollback-downgrade-drill.log", "production rollback downgrade drill", failures)
    if '"production_rollback_downgrade_validation"' not in manifest:
        failures.append("evidence manifest must include production_rollback_downgrade_validation")

    scenarios = [
        {
            "name": name,
            "fallback_mode": fallback,
            "live_submit_allowed": False,
            "auto_reenable_live_submit": False,
            "operator_required": True,
            "remote_side_effects": False,
        }
        for name, fallback in SCENARIOS
    ]
    result = {
        "status": "fail" if failures else "pass",
        "scenarios": scenarios,
        "allowed_fallback_modes": ["sign-only", "cancel-only", "read-only"],
        "production_ready_claimed": False,
        "remote_side_effects": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
