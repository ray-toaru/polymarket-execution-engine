#!/usr/bin/env python3
"""Validate real-funds canary program readiness without executing a live order."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter"
CLI = ADAPTER / "src" / "bin" / "pmx-real-funds-canary.rs"
LIVE_CANARY = ADAPTER / "src" / "sdk_runtime" / "live_canary.rs"
CARGO = ADAPTER / "Cargo.toml"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_ALLOW_REAL_FUNDS_CANARY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during readiness drill")

    require_current_gate_log(
        "67-real-funds-canary-ready-drill.log",
        "real funds canary readiness drill",
        failures,
    )
    writer = MANIFEST_WRITER.read_text()
    if '"real_funds_canary_ready_validation"' not in writer:
        failures.append("evidence manifest must include real_funds_canary_ready_validation")
    if "67-real-funds-canary-ready-drill.log" not in writer:
        failures.append("evidence manifest must capture real-funds canary readiness log")

    cargo = CARGO.read_text()
    for token in [
        'name = "pmx-real-funds-canary"',
        'required-features = ["live-submit"]',
    ]:
        if token not in cargo:
            failures.append(f"adapter Cargo.toml missing CLI token: {token}")

    cli = CLI.read_text() if CLI.exists() else ""
    if not cli:
        failures.append("real-funds canary CLI missing")
    for token in [
        "--dry-run",
        "--armed",
        "--market-file",
        "--release-decision-file",
        "run_real_funds_canary_gtc_post_only_cancel",
        "validate_real_funds_canary_market_with_diagnostics",
        "PMX_ALLOW_LIVE_SUBMIT",
        "PMX_ALLOW_REAL_FUNDS_CANARY",
        "PMX_BALANCE_ALLOWANCE_CHECKED",
        "dry_run_blocked_unsafe_market_candidate",
        "release_decision_bound",
        "posted: false",
        "remote_side_effects: false",
        "raw_signed_order_exposed: false",
    ]:
        if token not in cli:
            failures.append(f"real-funds canary CLI missing token: {token}")
    if ".post_order(" in cli or ".post_orders(" in cli:
        failures.append("real-funds canary CLI must not call post_order directly")

    live = LIVE_CANARY.read_text() if LIVE_CANARY.exists() else ""
    for token in [
        "OrderBookSummaryRequest",
        "SpreadRequest",
        "select_real_funds_canary_market_with_diagnostics",
    ]:
        if token not in live:
            failures.append(f"live canary market validation missing token: {token}")
    forbidden_live_tokens = [
        "simplified_markets",
        "markets(",
        "sampling_markets",
        "sampling_simplified_markets",
    ]
    for token in forbidden_live_tokens:
        if token in live:
            failures.append(f"live canary runtime must not perform active market discovery: {token}")
    if len(re.findall(r"\.\s*post_order\s*\(", live)) != 1:
        failures.append("live canary SDK runtime must still contain exactly one dedicated canary post_order call")

    result = {
        "status": "fail" if failures else "pass",
        "program_ready": not failures,
        "actual_execution_performed": False,
        "posted": False,
        "remote_side_effects": False,
        "cli_defaults_to_dry_run": True,
        "entrypoint": "pmx-real-funds-canary",
        "market_selection": "external-candidate-clob-validation",
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
