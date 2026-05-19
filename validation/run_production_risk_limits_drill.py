#!/usr/bin/env python3
"""Validate local production risk-limit decision matrix."""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, replace
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_RISK_LIMITS_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"


@dataclass(frozen=True)
class RiskState:
    account_whitelist: bool = True
    market_whitelist: bool = True
    per_order_cap: bool = True
    per_day_cap: bool = True
    exposure_cap: bool = True
    operator_approval_threshold: bool = True
    remote_unknown_freeze_override: bool = False
    stale_market_data_blocks: bool = False
    geoblock_blocks: bool = False


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def evaluate(name: str, state: RiskState) -> dict[str, object]:
    ok = all(
        [
            state.account_whitelist,
            state.market_whitelist,
            state.per_order_cap,
            state.per_day_cap,
            state.exposure_cap,
            state.operator_approval_threshold,
            not state.remote_unknown_freeze_override,
            not state.stale_market_data_blocks,
            not state.geoblock_blocks,
        ]
    )
    return {
        "name": name,
        "checks": state.__dict__,
        "risk_local_ready": ok,
        "live_submit_allowed": False,
        "operator_required": not ok,
        "remote_side_effects": False,
    }


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during risk limits drill")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production risk limits drill document missing")
    for token in [
        "account_whitelist",
        "market_whitelist",
        "per_order_cap",
        "per_day_cap",
        "exposure_cap",
        "operator_approval_threshold",
        "remote_unknown_freeze_override",
        "stale_market_data_blocks",
        "geoblock_blocks",
        "live_submit_allowed = false",
        "remote_side_effects = false",
    ]:
        if token not in doc:
            failures.append(f"production risk limits document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("55-production-risk-limits-drill.log", "production risk limits drill", failures)
    if '"production_risk_limits_validation"' not in manifest:
        failures.append("evidence manifest must include production_risk_limits_validation")

    base = RiskState()
    scenarios = [
        evaluate("baseline_local_risk_ready_but_live_blocked", base),
        evaluate("account_not_whitelisted", replace(base, account_whitelist=False)),
        evaluate("market_not_whitelisted", replace(base, market_whitelist=False)),
        evaluate("per_order_cap_exceeded", replace(base, per_order_cap=False)),
        evaluate("per_day_cap_exceeded", replace(base, per_day_cap=False)),
        evaluate("exposure_cap_exceeded", replace(base, exposure_cap=False)),
        evaluate("operator_threshold_missing", replace(base, operator_approval_threshold=False)),
        evaluate("remote_unknown_freeze", replace(base, remote_unknown_freeze_override=True)),
        evaluate("stale_market_data", replace(base, stale_market_data_blocks=True)),
        evaluate("geoblocked", replace(base, geoblock_blocks=True)),
    ]
    result = {
        "status": "fail" if failures else "pass",
        "scenarios": scenarios,
        "live_submit_allowed": False,
        "production_ready_claimed": False,
        "remote_side_effects": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
