#!/usr/bin/env python3
"""Simulate reconciliation drift and require fail-closed handling."""
from __future__ import annotations

import json
from datetime import datetime, timezone


def main() -> int:
    local_order = {
        "order_id": "shadow-local-order",
        "lifecycle_state": "POST_REQUESTED",
        "remote_order_id": "remote-shadow-order",
    }
    remote_order = {
        "remote_order_id": "remote-shadow-order",
        "observed_state": "MISSING",
    }
    drift = local_order["lifecycle_state"] == "POST_REQUESTED" and remote_order["observed_state"] == "MISSING"
    decision = {
        "schema_version": 1,
        "status": "pass" if drift else "fail",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "drill": "reconciliation_drift",
        "remote_side_effects": False,
        "local_state": local_order,
        "remote_observation": remote_order,
        "reconcile_decision": {
            "drift_detected": drift,
            "resulting_lifecycle_state": "REMOTE_UNKNOWN" if drift else "UNCHANGED",
            "submit_allowed": False,
            "reason": "remote missing after local post-request must fail closed",
        },
    }
    print(json.dumps(decision, indent=2, sort_keys=True))
    return 0 if drift else 1


if __name__ == "__main__":
    raise SystemExit(main())
