#!/usr/bin/env python3
"""Run a dry-run rehearsal of the future live canary sequence without side effects."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_REHEARSAL_DRILL.md"
ALLOWED_CANARY_POST_ORDER = ADAPTER / "sdk_runtime" / "live_canary.rs"

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
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_order\s*\("),
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
    failures: list[str] = []
    adapter = read_rust_sources(ADAPTER)
    stripped = strip_rust_comments(adapter)
    for token in REQUIRED_TOKENS:
        if token not in adapter:
            failures.append(f"adapter missing rehearsal token: {token}")
    for pattern in FORBIDDEN_CALLS:
        if pattern.search(stripped):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")
    post_order_call_sites = [
        source
        for source in rust_sources(ADAPTER)
        if POST_ORDER_CALL.search(strip_rust_comments(source.read_text()))
    ]
    if post_order_call_sites != [ALLOWED_CANARY_POST_ORDER]:
        display = ", ".join(str(path.relative_to(ADAPTER)) for path in post_order_call_sites) or "none"
        failures.append(
            "post_order call sites must be limited to guarded real-funds canary, "
            f"not rehearsal-drill paths; found {display}"
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

    manifest = MANIFEST.read_text()
    require_current_gate_log("40-live-canary-rehearsal-drill.log", "live canary rehearsal drill", failures)
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
