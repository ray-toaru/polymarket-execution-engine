#!/usr/bin/env python3
"""Run a dry-run rehearsal of the future live canary sequence without side effects."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "lib.rs"
GATES = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST = ROOT / "validation" / "write_v0_23_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_REHEARSAL_DRILL.md"

REHEARSAL_STAGES = [
    "whitelist_check",
    "caps_check",
    "operator_approval_check",
    "reservation_check",
    "idempotency_check",
    "reconcile_check",
    "remote_unknown_freeze_check",
    "post_submit_reconcile_check",
    "cancel_unknown_escalation_check",
    "cancel_only_fallback_check",
]

REQUIRED_TOKENS = [
    "prepare_live_canary_decision",
    "validate_live_submit_canary_preconditions",
    "remote unknown freeze active",
    "cancel_only_fallback_ready",
    "live_canary_prep_requires_whitelist_caps_approval_and_cancel_fallback",
]

FORBIDDEN_CALLS = [
    re.compile(r"\.\s*post_order\s*\("),
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_order\s*\("),
    re.compile(r"\.\s*cancel_orders\s*\("),
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"//.*", "", text)
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def main() -> int:
    failures: list[str] = []
    adapter = ADAPTER.read_text()
    stripped = strip_rust_comments(adapter)
    for token in REQUIRED_TOKENS:
        if token not in adapter:
            failures.append(f"adapter missing rehearsal token: {token}")
    for pattern in FORBIDDEN_CALLS:
        if pattern.search(stripped):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")
    for env_name in [
        "PMX_ALLOW_LIVE_SUBMIT",
        "PMX_ALLOW_LIVE_CANCEL",
        "PMX_OPERATOR_APPROVED_LIVE_CANARY",
    ]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is not allowed during dry-run rehearsal")

    if not DOC.exists():
        failures.append("live canary rehearsal drill document missing")
    else:
        doc = DOC.read_text()
        for token in ["blocked_dry_run", "no live submit", "no live cancel", *REHEARSAL_STAGES]:
            if token not in doc:
                failures.append(f"live canary rehearsal document missing token: {token}")

    gates = GATES.read_text()
    manifest = MANIFEST.read_text()
    if "40-live-canary-rehearsal-drill.log" not in gates:
        failures.append("run_v0_23_gates.sh must emit live canary rehearsal drill log")
    if '"live_canary_rehearsal_validation"' not in manifest:
        failures.append("evidence manifest must include live_canary_rehearsal_validation")
    if "40-live-canary-rehearsal-drill.log" not in manifest:
        failures.append("evidence manifest must capture live canary rehearsal drill log")

    result = {
        "status": "fail" if failures else "pass",
        "rehearsal_status": "blocked_dry_run",
        "stages": REHEARSAL_STAGES,
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
