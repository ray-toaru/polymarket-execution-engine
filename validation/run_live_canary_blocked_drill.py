#!/usr/bin/env python3
"""Prove the live canary execution shell remains blocked by default."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
CURRENT_GATES = ROOT / "validation" / "run_current_gates.sh"
IMPLEMENTATION_GATES = ROOT / "validation" / "run_v0_24_gates.sh"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_BLOCKED_DRILL.md"

REQUIRED_TOKENS = [
    "prepare_live_canary_decision",
    "validate_live_submit_canary_preconditions",
    "remote unknown freeze active",
    "live_canary_prep_freezes_on_remote_unknown_and_never_submits",
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


def read_rust_sources(path: Path) -> str:
    return "\n".join(source.read_text() for source in sorted(path.rglob("*.rs")))


def main() -> int:
    adapter = read_rust_sources(ADAPTER)
    stripped = strip_rust_comments(adapter)
    failures: list[str] = []
    for token in REQUIRED_TOKENS:
        if token not in adapter:
            failures.append(f"adapter missing blocked canary token: {token}")
    for pattern in FORBIDDEN_CALLS:
        if pattern.search(stripped):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")
    if not DOC.exists():
        failures.append("live canary blocked drill document missing")
    current_gates = CURRENT_GATES.read_text()
    implementation_gates = IMPLEMENTATION_GATES.read_text()
    manifest = MANIFEST.read_text()
    if IMPLEMENTATION_GATES.name not in current_gates:
        failures.append("run_current_gates.sh must delegate to the active gate implementation")
    if "39-live-canary-blocked-drill.log" not in implementation_gates:
        failures.append("current gates must emit live canary blocked drill log")
    if '"live_canary_blocked_validation"' not in manifest:
        failures.append("evidence manifest must include live_canary_blocked_validation")
    if "39-live-canary-blocked-drill.log" not in manifest:
        failures.append("evidence manifest must capture live canary blocked drill log")

    live_submit = env_enabled("PMX_ALLOW_LIVE_SUBMIT")
    live_cancel = env_enabled("PMX_ALLOW_LIVE_CANCEL")
    operator_approved = env_enabled("PMX_OPERATOR_APPROVED_LIVE_CANARY")
    if live_submit or live_cancel or operator_approved:
        failures.append("blocked canary drill must run with live submit/cancel/operator approval disabled")

    result = {
        "status": "fail" if failures else "pass",
        "canary_status": "blocked",
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "live_submit_env_enabled": live_submit,
        "live_cancel_env_enabled": live_cancel,
        "operator_approved_live_canary": operator_approved,
        "blocked_reason": "live canary requires a future reviewed release decision and explicit side-effect gates",
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
