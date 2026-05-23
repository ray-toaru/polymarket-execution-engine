#!/usr/bin/env python3
"""Emit real-funds canary preflight evidence without remote side effects."""
from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "REAL_FUNDS_CANARY.md"
APPROVAL = ROOT / "config" / "real-funds-canary.approval.example.json"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
ADAPTER_SRC = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
LIVE_CANARY_SRC = ADAPTER_SRC / "sdk_runtime" / "live_canary.rs"

FORBIDDEN_APPROVAL_TERMS = [
    "private_key",
    "clob_secret",
    "api_secret",
    "raw_signature",
    "raw_signed_payload",
    "SignedOrderEnvelope",
]

DOC_TOKENS = [
    "REAL_FUNDS_CANARY",
    "GTC_LIMIT_POST_ONLY_CANCEL",
    "PMX_ALLOW_REAL_FUNDS_CANARY",
    "allow_real_funds_canary = true",
    "approval_file_required = true",
    "artifact_sha256_required = true",
    "evidence_manifest_sha256_required = true",
    "max_order_notional_usd = 1",
    "max_daily_notional_usd = 5",
    "target_size_is_reviewed_candidate_input = true",
    "notional_usd_is_price_times_size = true",
    "limit_order_size_driven = true",
    "runtime_truth_file_required = true",
    "runtime_truth_store_projection_available = true",
    "external_candidate_market_required = true",
    "engine_market_discovery_allowed = false",
    "live_submit_allowed = false",
    "live_cancel_allowed = false",
    "real_funds_canary_allowed = false",
    "posted = false",
    "remote_side_effects = false",
    "raw_signed_order_logged = false",
    "raw_signed_order_exposed = false",
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def read_adapter_sources() -> str:
    return "\n".join(path.read_text() for path in sorted(ADAPTER_SRC.rglob("*.rs")))


def is_sha256(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(ch in "0123456789abcdefABCDEF" for ch in value)


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_ALLOW_REAL_FUNDS_CANARY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during normal real-funds canary preflight")

    require_current_gate_log(
        "65-real-funds-canary-preflight.log",
        "real funds canary preflight drill",
        failures,
    )
    manifest_writer = MANIFEST_WRITER.read_text()
    if '"real_funds_canary_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include real_funds_canary_preflight_validation")
    if "65-real-funds-canary-preflight.log" not in manifest_writer:
        failures.append("evidence manifest must capture real funds canary preflight log")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("real funds canary document missing")
    for token in DOC_TOKENS:
        if token not in doc:
            failures.append(f"real funds canary document missing token: {token}")

    if not APPROVAL.exists():
        failures.append("real funds canary approval example missing")
        approval: dict[str, Any] = {}
    else:
        approval = load_json(APPROVAL)
        approval_text = APPROVAL.read_text()
        for term in FORBIDDEN_APPROVAL_TERMS:
            if term in approval_text:
                failures.append(f"approval example contains forbidden sensitive term: {term}")
        if approval.get("scope") != "REAL_FUNDS_CANARY":
            failures.append("approval example must use REAL_FUNDS_CANARY scope")
        if approval.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
            failures.append("approval example must use GTC_LIMIT_POST_ONLY_CANCEL")
        if approval.get("max_order_notional_usd") != "1":
            failures.append("approval example must cap each order at 1 USD")
        if approval.get("max_daily_notional_usd") != "5":
            failures.append("approval example must cap daily notional at 5 USD")
        for key in ["approval_hash", "artifact_sha256", "evidence_manifest_sha256"]:
            if not is_sha256(approval.get(key)):
                failures.append(f"approval example must include 64-hex {key}")

    adapter_text = read_adapter_sources()
    for token in [
        "RealFundsCanaryPreconditions",
        "ENV_ALLOW_REAL_FUNDS_CANARY",
        "load_runtime_truth_file",
        "load_canary_runtime_truth",
        "--runtime-truth-store",
        "durable_runtime_truth",
        "validate_real_funds_canary_preconditions",
        "runtime_kill_switch_truth_bound",
        "runtime_live_submit_gate_bound",
        "runtime_idempotency_lease_bound",
        "runtime_order_cancel_reconciliation_bound",
        "run_real_funds_canary_gtc_post_only_cancel",
        "limit_order()",
        "size(size)",
        "SdkOrderType::GTC",
        "raw_signed_order_exposed: false",
    ]:
        if token not in adapter_text:
            failures.append(f"adapter source missing real-funds canary guard token: {token}")
    if "post_orders(" in adapter_text:
        failures.append("bulk post_orders must remain absent")
    live_canary_text = LIVE_CANARY_SRC.read_text() if LIVE_CANARY_SRC.exists() else ""
    if ".market_order(" in live_canary_text:
        failures.append("real-funds canary must use limit order size semantics, not market_order amount semantics")
    post_order_occurrences = adapter_text.count(".post_order(")
    gateway_text = (ADAPTER_SRC / "sdk_runtime" / "gateway.rs").read_text() if (ADAPTER_SRC / "sdk_runtime" / "gateway.rs").exists() else ""
    if post_order_occurrences != 2 or ".post_order(signed)" not in live_canary_text or ".post_order(signed)" not in gateway_text:
        failures.append("adapter post_order call sites must be limited to guarded canary and explicit SDK gateway bridge")

    result = {
        "status": "fail" if failures else "pass",
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_allowed": False,
        "posted": False,
        "remote_side_effects": False,
        "max_order_notional_usd": "1",
        "max_daily_notional_usd": "5",
        "target_size_is_reviewed_candidate_input": True,
        "notional_usd_is_price_times_size": True,
        "limit_order_size_driven": True,
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "approval_file_required": True,
        "artifact_hash_required": True,
        "evidence_manifest_hash_required": True,
        "external_candidate_market_required": True,
        "engine_market_discovery_allowed": False,
        "raw_signed_order_logged": False,
        "raw_signed_order_exposed": False,
        "approval_fixture": str(APPROVAL.relative_to(ROOT)),
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
