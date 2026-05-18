#!/usr/bin/env python3
"""Validate live canary readiness gates without enabling live side effects."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "lib.rs"
GATES = ROOT / "validation" / "run_v0_24_gates.sh"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_READINESS_DRILL.md"

REQUIRED_CANARY_GATES = [
    "compile_feature_live_submit",
    "env_allow_live_submit",
    "config_allow_live_submit",
    "kill_switch_open",
    "runtime_worker_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_worker_healthy",
    "account_whitelisted",
    "market_whitelisted",
    "size_cap_ok",
    "daily_cap_ok",
    "operator_approved",
    "cancel_only_fallback_ready",
]

REQUIRED_ADAPTER_TOKENS = [
    "LiveCanaryPreconditions",
    "LiveCanaryPrepInput",
    "LiveCanaryPrepDecision",
    "default_blocked_live_canary_preconditions",
    "prepare_live_canary_decision",
    "validate_live_submit_canary_preconditions",
    "live_canary_default_preconditions_are_blocked_without_side_effects",
    "live_canary_prep_freezes_on_remote_unknown_and_never_submits",
    "live_canary_prep_requires_whitelist_caps_approval_and_cancel_fallback",
    "live_submit_canary_requires_every_gate",
    "live_submit_canary_requires_cancel_only_fallback",
    "ENV_ALLOW_LIVE_SUBMIT",
    "ENV_ALLOW_LIVE_CANCEL",
]

REQUIRED_DOC_TOKENS = [
    "compile feature",
    "environment gate",
    "kill switch",
    "runtime worker",
    "repository reservation",
    "idempotency key",
    "operator approval",
    "cancel-only fallback",
    "no live submit",
    "no live cancel",
]

FORBIDDEN_SIDE_EFFECT_CALLS = [
    re.compile(r"\.\s*post_order\s*\("),
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_order\s*\("),
    re.compile(r"\.\s*cancel_orders\s*\("),
]


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"//.*", "", text)
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    adapter = ADAPTER.read_text()
    stripped_adapter = strip_rust_comments(adapter)
    for token in REQUIRED_ADAPTER_TOKENS + REQUIRED_CANARY_GATES:
        if token not in adapter:
            failures.append(f"adapter missing live canary readiness token: {token}")
    for pattern in FORBIDDEN_SIDE_EFFECT_CALLS:
        if pattern.search(stripped_adapter):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is not allowed during readiness drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is not allowed during readiness drill")

    if not DOC.exists():
        failures.append("live canary readiness drill document missing")
    else:
        doc = DOC.read_text().lower()
        for token in REQUIRED_DOC_TOKENS:
            if token not in doc:
                failures.append(f"live canary readiness drill document missing token: {token}")

    gates = GATES.read_text()
    manifest = MANIFEST.read_text()
    if "38-live-canary-readiness-drill.log" not in gates:
        failures.append("run_v0_24_gates.sh must emit live canary readiness drill log")
    if '"live_canary_readiness_validation"' not in manifest:
        failures.append("evidence manifest must include live_canary_readiness_validation")
    if "38-live-canary-readiness-drill.log" not in manifest:
        failures.append("evidence manifest must capture live canary readiness drill log")

    result = {
        "status": "fail" if failures else "pass",
        "live_submit_env_enabled": env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_env_enabled": env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "required_gates": REQUIRED_CANARY_GATES,
        "live_side_effects": "not_executed",
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
