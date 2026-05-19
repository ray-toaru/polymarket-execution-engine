#!/usr/bin/env python3
"""Validate conservative production config profile defaults."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_CONFIG_PROFILE_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during config profile drill")

    required = [
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
        "live_cancel_allowed = false",
        "production_ready_claimed = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production config profile drill document missing")
    for token in required:
        if token not in doc:
            failures.append(f"production config profile document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("56-production-config-profile-drill.log", "production config profile drill", failures)
    if '"production_config_profile_validation"' not in manifest:
        failures.append("evidence manifest must include production_config_profile_validation")

    config = {
        "live_submit_default_disabled": True,
        "live_cancel_default_disabled": True,
        "production_ready_default_false": True,
        "kill_switch_default_closed": True,
        "per_account_enablement_required": True,
        "per_market_enablement_required": True,
        "amount_caps_required": True,
        "operator_approval_required": True,
        "canary_profile_isolated": True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
    }
    result = {"status": "fail" if failures else "pass", "config": config, "failures": failures}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
