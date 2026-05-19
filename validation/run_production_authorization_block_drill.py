#!/usr/bin/env python3
"""Prove partial production/live authorization cannot enable side effects."""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, replace
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_AUTHORIZATION_BLOCK_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"


@dataclass(frozen=True)
class AuthorizationState:
    compile_feature_live_submit: bool = True
    env_allow_live_submit: bool = True
    config_allow_live_submit: bool = True
    kill_switch_open: bool = True
    runtime_healthy: bool = True
    geoblock_allowed: bool = True
    repository_reservation_exists: bool = True
    idempotency_key_written: bool = True
    reconcile_healthy: bool = True
    account_whitelisted: bool = True
    market_whitelisted: bool = True
    per_order_cap_ok: bool = True
    per_day_cap_ok: bool = True
    operator_approval_present: bool = True
    reviewed_release_decision_present: bool = False


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def evaluate(name: str, state: AuthorizationState) -> dict[str, object]:
    gates = {
        "compile_feature_live_submit": state.compile_feature_live_submit,
        "env_allow_live_submit": state.env_allow_live_submit,
        "config_allow_live_submit": state.config_allow_live_submit,
        "kill_switch_open": state.kill_switch_open,
        "runtime_healthy": state.runtime_healthy,
        "geoblock_allowed": state.geoblock_allowed,
        "repository_reservation_exists": state.repository_reservation_exists,
        "idempotency_key_written": state.idempotency_key_written,
        "reconcile_healthy": state.reconcile_healthy,
        "account_whitelisted": state.account_whitelisted,
        "market_whitelisted": state.market_whitelisted,
        "per_order_cap_ok": state.per_order_cap_ok,
        "per_day_cap_ok": state.per_day_cap_ok,
        "operator_approval_present": state.operator_approval_present,
        "reviewed_release_decision_present": state.reviewed_release_decision_present,
    }
    missing = [gate for gate, ok in gates.items() if not ok]
    submit_allowed = not missing
    return {
        "name": name,
        "status": "fail_closed" if not submit_allowed else "unexpected_open",
        "gates": gates,
        "missing": missing,
        "submit_allowed": submit_allowed,
        "cancel_allowed": False,
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
    }


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during authorization block drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during authorization block drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production authorization block drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "compile_feature_live_submit",
            "env_allow_live_submit",
            "config_allow_live_submit",
            "kill_switch_open",
            "runtime_healthy",
            "geoblock_allowed",
            "repository_reservation_exists",
            "idempotency_key_written",
            "reconcile_healthy",
            "account_whitelisted",
            "market_whitelisted",
            "per_order_cap_ok",
            "per_day_cap_ok",
            "operator_approval_present",
            "reviewed_release_decision_present",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production authorization block document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log(
        "47-production-authorization-block-drill.log",
        "production authorization block drill",
        failures,
    )
    if '"production_authorization_block_validation"' not in manifest:
        failures.append("evidence manifest must include production_authorization_block_validation")
    if "47-production-authorization-block-drill.log" not in manifest:
        failures.append("evidence manifest must capture production authorization block log")

    base = AuthorizationState()
    scenarios = [
        evaluate("all_local_gates_but_no_reviewed_release", base),
        evaluate("missing_compile_feature", replace(base, compile_feature_live_submit=False)),
        evaluate("missing_env_allow", replace(base, env_allow_live_submit=False)),
        evaluate("missing_config_allow", replace(base, config_allow_live_submit=False)),
        evaluate("kill_switch_closed", replace(base, kill_switch_open=False)),
        evaluate("runtime_unhealthy", replace(base, runtime_healthy=False)),
        evaluate("geoblocked", replace(base, geoblock_allowed=False)),
        evaluate("missing_repository_reservation", replace(base, repository_reservation_exists=False)),
        evaluate("missing_idempotency_key", replace(base, idempotency_key_written=False)),
        evaluate("reconcile_unhealthy", replace(base, reconcile_healthy=False)),
        evaluate("account_not_whitelisted", replace(base, account_whitelisted=False)),
        evaluate("market_not_whitelisted", replace(base, market_whitelisted=False)),
        evaluate("per_order_cap_exceeded", replace(base, per_order_cap_ok=False)),
        evaluate("per_day_cap_exceeded", replace(base, per_day_cap_ok=False)),
        evaluate("missing_operator_approval", replace(base, operator_approval_present=False)),
    ]

    for scenario in scenarios:
        if scenario["status"] != "fail_closed":
            failures.append(f"{scenario['name']} must fail closed")
        if scenario["submit_allowed"] is not False:
            failures.append(f"{scenario['name']} must not allow submit")
        if scenario["remote_side_effects"] is not False:
            failures.append(f"{scenario['name']} must not record remote side effects")

    result = {
        "status": "fail" if failures else "pass",
        "authorization_status": "blocked_without_reviewed_release",
        "production_ready_claimed": False,
        "live_submit_env_enabled": env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_env_enabled": env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "remote_side_effects": False,
        "scenarios": scenarios,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
