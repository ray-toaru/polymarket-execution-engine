#!/usr/bin/env python3
"""Exercise rollback and kill-switch fail-closed scenarios without side effects."""
from __future__ import annotations

import json
from datetime import datetime, timezone


SCENARIOS = [
    {
        "name": "config_kill_switch_closed",
        "kill_switch_open": False,
        "runtime_status": "READY",
        "sdk_status": "READY",
        "postgres_status": "READY",
    },
    {
        "name": "runtime_degraded",
        "kill_switch_open": True,
        "runtime_status": "DEGRADED",
        "sdk_status": "READY",
        "postgres_status": "READY",
    },
    {
        "name": "sdk_failure",
        "kill_switch_open": True,
        "runtime_status": "READY",
        "sdk_status": "FAILED",
        "postgres_status": "READY",
    },
    {
        "name": "postgres_unavailable",
        "kill_switch_open": True,
        "runtime_status": "READY",
        "sdk_status": "READY",
        "postgres_status": "UNAVAILABLE",
    },
    {
        "name": "geoblock_blocked",
        "kill_switch_open": True,
        "runtime_status": "GEOBLOCKED",
        "sdk_status": "READY",
        "postgres_status": "READY",
    },
    {
        "name": "low_resource",
        "kill_switch_open": True,
        "runtime_status": "LOW_RESOURCE",
        "sdk_status": "READY",
        "postgres_status": "READY",
    },
    {
        "name": "remote_unknown_freeze",
        "kill_switch_open": True,
        "runtime_status": "REMOTE_UNKNOWN_FREEZE",
        "sdk_status": "READY",
        "postgres_status": "READY",
    },
]


def submit_allowed(scenario: dict[str, object]) -> bool:
    return (
        scenario["kill_switch_open"] is True
        and scenario["runtime_status"] == "READY"
        and scenario["sdk_status"] == "READY"
        and scenario["postgres_status"] == "READY"
    )


def main() -> int:
    results = []
    for scenario in SCENARIOS:
        allowed = submit_allowed(scenario)
        results.append(
            {
                **scenario,
                "submit_allowed": allowed,
                "rollback_action": "BLOCK_SUBMIT_AND_KEEP_PREVIOUS_SAFE_STATE",
                "fallback_mode": "sign-only" if scenario["sdk_status"] == "FAILED" else "read-only",
                "operator_required": True,
            }
        )
    failures = [result["name"] for result in results if result["submit_allowed"]]
    report = {
        "schema_version": 1,
        "status": "pass" if not failures else "fail",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "drill": "rollback_kill_switch",
        "remote_side_effects": False,
        "posted": False,
        "cancelled": False,
        "scenarios": results,
        "failed_scenarios": failures,
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
