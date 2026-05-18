#!/usr/bin/env python3
"""Guard shadow and rollback drill invariants without network access."""
from __future__ import annotations

import json

from run_kill_switch_rollback_drill import build_rollback_report
from run_shadow_execution_drill import (
    build_shadow_decision,
    decimal_string,
    validate_shadow_decision,
)


def main() -> int:
    failures: list[str] = []
    market = {
        "condition_id": "0xcondition",
        "active": True,
        "archived": False,
        "closed": False,
        "accepting_orders": True,
        "tokens": [{"token_id": "12345"}],
    }
    shadow = build_shadow_decision(
        market=market,
        size=decimal_string("5", "size"),
        limit_price=decimal_string("0.01", "limit_price"),
        sensitive_env_present=True,
    )
    failures.extend(validate_shadow_decision(shadow))
    if shadow["safety"]["sensitive_env_present"] is not True:
        failures.append("shadow guard must preserve sensitive-env-present signal without logging secrets")
    encoded_shadow = json.dumps(shadow)
    if "12345" in encoded_shadow or "0xcondition" in encoded_shadow:
        failures.append("shadow guard leaked raw token or condition id")

    rollback = build_rollback_report()
    if rollback["status"] != "pass":
        failures.append("rollback report must pass with all modeled fail-closed scenarios")
    if rollback["remote_side_effects"] is not False or rollback["posted"] is not False:
        failures.append("rollback drill must remain non-posting and side-effect free")
    scenario_names = {scenario["name"] for scenario in rollback["scenarios"]}
    for required in {
        "config_kill_switch_closed",
        "runtime_degraded",
        "sdk_failure",
        "postgres_unavailable",
        "geoblock_blocked",
        "low_resource",
        "remote_unknown_freeze",
    }:
        if required not in scenario_names:
            failures.append(f"rollback drill missing {required}")

    result = {
        "status": "fail" if failures else "pass",
        "remote_side_effects": False,
        "checks": [
            "shadow_non_posting",
            "shadow_hashed_identifiers",
            "shadow_sensitive_env_redacted",
            "rollback_fail_closed",
            "rollback_fallback_modes",
        ],
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
