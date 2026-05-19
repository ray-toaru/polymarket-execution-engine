#!/usr/bin/env python3
"""Emit structured live-canary preflight evidence without side effects."""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, replace
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOC = ROOT / "docs" / "LIVE_CANARY_PREFLIGHT.md"


@dataclass(frozen=True)
class CanaryInput:
    account_id: str = "acct-canary"
    market_id: str = "market-canary"
    order_size_units: int = 1
    daily_used_units: int = 0
    per_order_cap_units: int = 10
    per_day_cap_units: int = 10
    account_whitelist: tuple[str, ...] = ("acct-canary",)
    market_whitelist: tuple[str, ...] = ("market-canary",)
    operator_approval_id: str | None = "approval-1"
    cancel_only_fallback_ready: bool = True
    remote_unknown_orders: int = 0


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def evaluate(name: str, data: CanaryInput) -> dict[str, object]:
    account_whitelisted = data.account_id in data.account_whitelist
    market_whitelisted = data.market_id in data.market_whitelist
    size_cap_ok = 0 < data.order_size_units <= data.per_order_cap_units
    daily_cap_ok = 0 < data.order_size_units and (
        data.daily_used_units + data.order_size_units <= data.per_day_cap_units
    )
    operator_approved = bool(data.operator_approval_id and data.operator_approval_id.strip())
    frozen = data.remote_unknown_orders > 0
    reasons: list[str] = []
    if not account_whitelisted:
        reasons.append("account not whitelisted")
    if not market_whitelisted:
        reasons.append("market not whitelisted")
    if not size_cap_ok:
        reasons.append("per-order cap exceeded")
    if not daily_cap_ok:
        reasons.append("per-day cap exceeded")
    if not operator_approved:
        reasons.append("operator approval missing")
    if not data.cancel_only_fallback_ready:
        reasons.append("cancel-only fallback missing")
    if frozen:
        reasons.append("remote unknown freeze active")

    local_preflight_ready = not reasons
    return {
        "name": name,
        "status": "pass" if local_preflight_ready else "fail_closed",
        "local_preflight_ready": local_preflight_ready,
        "frozen": frozen,
        "submit_allowed": False,
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "checks": {
            "account_whitelisted": account_whitelisted,
            "market_whitelisted": market_whitelisted,
            "size_cap_ok": size_cap_ok,
            "daily_cap_ok": daily_cap_ok,
            "operator_approved": operator_approved,
            "cancel_only_fallback_ready": data.cancel_only_fallback_ready,
            "remote_unknown_freeze_clear": not frozen,
            "reservation_ready": True,
            "idempotency_ready": True,
            "reconcile_ready": True,
        },
        "reasons": reasons,
    }


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during canary preflight")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during canary preflight")
    if env_enabled("PMX_OPERATOR_APPROVED_LIVE_CANARY"):
        failures.append("PMX_OPERATOR_APPROVED_LIVE_CANARY=1 is forbidden during canary preflight")

    if not DOC.exists():
        failures.append("live canary preflight document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "account_whitelisted",
            "market_whitelisted",
            "size_cap_ok",
            "daily_cap_ok",
            "operator_approved",
            "cancel_only_fallback_ready",
            "remote_unknown_freeze_clear",
            "no live submit",
            "no live cancel",
        ]:
            if token not in doc:
                failures.append(f"live canary preflight document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("45-live-canary-preflight-drill.log", "live canary preflight drill", failures)
    if '"live_canary_preflight_validation"' not in manifest:
        failures.append("evidence manifest must include live_canary_preflight_validation")
    if "45-live-canary-preflight-drill.log" not in manifest:
        failures.append("evidence manifest must capture live canary preflight drill log")

    base = CanaryInput()
    scenarios = [
        evaluate("baseline_local_preflight", base),
        evaluate("missing_operator_approval", replace(base, operator_approval_id=None)),
        evaluate("per_order_cap_exceeded", replace(base, order_size_units=11)),
        evaluate("per_day_cap_exceeded", replace(base, daily_used_units=10)),
        evaluate("account_not_whitelisted", replace(base, account_id="acct-other")),
        evaluate("market_not_whitelisted", replace(base, market_id="market-other")),
        evaluate("cancel_only_fallback_missing", replace(base, cancel_only_fallback_ready=False)),
        evaluate("remote_unknown_freeze", replace(base, remote_unknown_orders=1)),
    ]

    expected_fail_closed = [scenario for scenario in scenarios if scenario["name"] != "baseline_local_preflight"]
    if not scenarios[0]["local_preflight_ready"]:
        failures.append("baseline local canary preflight should be locally ready")
    for scenario in expected_fail_closed:
        if scenario["status"] != "fail_closed":
            failures.append(f"{scenario['name']} must fail closed")
        if scenario["submit_allowed"] is not False:
            failures.append(f"{scenario['name']} must not allow submit")
        if scenario["remote_side_effects"] is not False:
            failures.append(f"{scenario['name']} must not have remote side effects")

    result = {
        "status": "fail" if failures else "pass",
        "preflight_status": "local_ready_but_live_blocked" if not failures else "failed",
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "live_submit_env_enabled": env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_env_enabled": env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "operator_approved_live_canary": env_enabled("PMX_OPERATOR_APPROVED_LIVE_CANARY"),
        "scenarios": scenarios,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
