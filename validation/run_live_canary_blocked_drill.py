#!/usr/bin/env python3
"""Prove the live canary execution shell remains blocked by default."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_BLOCKED_DRILL.md"
ALLOWED_CANARY_POST_ORDER = ADAPTER / "sdk_runtime" / "live_canary.rs"
ALLOWED_GATEWAY_POST_ORDER = ADAPTER / "sdk_runtime" / "gateway.rs"

REQUIRED_TOKENS = [
    "prepare_live_canary_decision",
    "validate_live_submit_canary_preconditions",
    "remote unknown freeze active",
    "live_canary_prep_freezes_on_remote_unknown_and_never_submits",
]

FORBIDDEN_CALLS = [
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_orders\s*\("),
]
POST_ORDER_CALL = re.compile(r"\.\s*post_order\s*\(")


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"//.*", "", text)
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def read_rust_sources(path: Path) -> str:
    return "\n".join(source.read_text() for source in sorted(path.rglob("*.rs")))


def rust_sources(path: Path) -> list[Path]:
    return sorted(path.rglob("*.rs"))


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
    post_order_call_sites = [
        source
        for source in rust_sources(ADAPTER)
        if POST_ORDER_CALL.search(strip_rust_comments(source.read_text()))
    ]
    allowed_post_order_sites = [ALLOWED_GATEWAY_POST_ORDER, ALLOWED_CANARY_POST_ORDER]
    if post_order_call_sites != allowed_post_order_sites:
        display = ", ".join(str(path.relative_to(ADAPTER)) for path in post_order_call_sites) or "none"
        failures.append(
            "post_order call sites must be limited to guarded real-funds canary, "
            f"not blocked-drill paths; found {display}"
        )
    if ALLOWED_CANARY_POST_ORDER.exists():
        canary = strip_rust_comments(ALLOWED_CANARY_POST_ORDER.read_text())
        for token in [
            "validate_real_funds_canary_preconditions",
            "SdkOrderType::FOK",
            "raw_signed_order_exposed: false",
        ]:
            if token not in canary:
                failures.append(f"guarded canary post_order site missing token: {token}")
    if not DOC.exists():
        failures.append("live canary blocked drill document missing")
    manifest = MANIFEST.read_text()
    require_current_gate_log("39-live-canary-blocked-drill.log", "live canary blocked drill", failures)
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
