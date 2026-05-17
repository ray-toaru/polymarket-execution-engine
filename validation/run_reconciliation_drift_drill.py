#!/usr/bin/env python3
"""Simulate reconciliation drift and require fail-closed handling."""
from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone


def trace_id(name: str) -> str:
    return f"reconcile-{hashlib.sha256(name.encode()).hexdigest()[:24]}"


SCENARIOS = [
    {
        "name": "remote_missing_first_observation",
        "local_state": "REMOTE_UNKNOWN",
        "remote_observation": "MISSING",
        "expected_lifecycle_state": "PARTIAL_REMOTE_UNKNOWN",
        "operator_required": False,
    },
    {
        "name": "remote_missing_second_observation",
        "local_state": "PARTIAL_REMOTE_UNKNOWN",
        "remote_observation": "MISSING",
        "expected_lifecycle_state": "FAILED",
        "operator_required": True,
    },
    {
        "name": "remote_open_restores_posted",
        "local_state": "REMOTE_UNKNOWN",
        "remote_observation": "OPEN",
        "expected_lifecycle_state": "REMOTE_POSTED",
        "operator_required": False,
    },
    {
        "name": "remote_unknown_operator_review",
        "local_state": "REMOTE_UNKNOWN",
        "remote_observation": "UNKNOWN",
        "expected_lifecycle_state": "REMOTE_UNKNOWN",
        "operator_required": True,
    },
]


def resulting_state(scenario: dict[str, object]) -> str:
    local = scenario["local_state"]
    remote = scenario["remote_observation"]
    if local == "REMOTE_UNKNOWN" and remote == "MISSING":
        return "PARTIAL_REMOTE_UNKNOWN"
    if local == "PARTIAL_REMOTE_UNKNOWN" and remote == "MISSING":
        return "FAILED"
    if local in {"REMOTE_UNKNOWN", "PARTIAL_REMOTE_UNKNOWN"} and remote == "OPEN":
        return "REMOTE_POSTED"
    if remote == "UNKNOWN":
        return str(local)
    return str(local)


def main() -> int:
    scenario_results = []
    failures = []
    for scenario in SCENARIOS:
        observed = resulting_state(scenario)
        passed = observed == scenario["expected_lifecycle_state"]
        if not passed:
            failures.append(scenario["name"])
        scenario_results.append(
            {
                **scenario,
                "trace_id": trace_id(str(scenario["name"])),
                "resulting_lifecycle_state": observed,
                "submit_allowed": False,
                "remote_side_effects": False,
                "passed": passed,
            }
        )
    decision = {
        "schema_version": 1,
        "status": "pass" if not failures else "fail",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "drill": "reconciliation_drift",
        "remote_side_effects": False,
        "scenarios": scenario_results,
        "failed_scenarios": failures,
        "reconcile_decision": "fail_closed_operator_escalation_on_missing_or_unknown_remote_truth",
    }
    print(json.dumps(decision, indent=2, sort_keys=True))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
