#!/usr/bin/env python3
"""Validate controlled live canary preparation while remaining blocked."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "LIVE_CANARY_CONTROLLED_PREP_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

GATES = [
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
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_OPERATOR_APPROVED_LIVE_CANARY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during controlled canary prep drill")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("controlled live canary prep document missing")
    for token in GATES + [
        "canary_submit_allowed = false",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]:
        if token not in doc:
            failures.append(f"controlled live canary prep document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("58-live-canary-controlled-prep-drill.log", "controlled live canary prep drill", failures)
    if '"live_canary_controlled_prep_validation"' not in manifest:
        failures.append("evidence manifest must include live_canary_controlled_prep_validation")

    result = {
        "status": "fail" if failures else "pass",
        "canary_status": "controlled_prep_blocked_without_reviewed_release",
        "gates": {gate: True for gate in GATES},
        "reviewed_release_decision_present": False,
        "canary_submit_allowed": False,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
