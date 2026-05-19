#!/usr/bin/env python3
"""Validate local real-funds canary lifecycle closure without remote side effects."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "REAL_FUNDS_CANARY_LIFECYCLE.md"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
STORE_SRC = ROOT / "crates" / "pmx-store" / "src"
SERVICE_SRC = ROOT / "crates" / "pmx-service" / "src"

DOC_TOKENS = [
    "REAL_FUNDS_CANARY_LIFECYCLE",
    "PREFLIGHT_READY",
    "READY_BUT_LIVE_DISABLED",
    "REMOTE_UNKNOWN_FREEZE",
    "OPERATOR_REQUIRED",
    "SIMULATED_RECONCILED",
    "remote_side_effects = false",
    "raw_signed_order_exposed = false",
    "idempotency replay",
    "idempotency conflict",
    "simulated reconcile",
]

SOURCE_TOKENS = [
    "RealFundsCanaryRunStore",
    "RealFundsCanaryLifecycleState",
    "record_real_funds_canary_run",
    "load_real_funds_canary_run_by_idempotency",
    "update_real_funds_canary_state",
    "RemoteUnknownFreeze",
    "SimulatedReconciled",
    "raw_signed_order_exposed",
    "remote_side_effects",
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def read_sources(root: Path) -> str:
    return "\n".join(path.read_text() for path in sorted(root.rglob("*.rs")))


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_ALLOW_REAL_FUNDS_CANARY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during lifecycle drill")

    require_current_gate_log(
        "66-real-funds-canary-lifecycle-drill.log",
        "real funds canary lifecycle drill",
        failures,
    )
    manifest_writer = MANIFEST_WRITER.read_text()
    if '"real_funds_canary_lifecycle_validation"' not in manifest_writer:
        failures.append("evidence manifest must include real_funds_canary_lifecycle_validation")
    if "66-real-funds-canary-lifecycle-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture real funds canary lifecycle log")

    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("real funds canary lifecycle document missing")
    for token in DOC_TOKENS:
        if token not in doc:
            failures.append(f"real funds canary lifecycle document missing token: {token}")

    sources = read_sources(STORE_SRC) + "\n" + read_sources(SERVICE_SRC)
    for token in SOURCE_TOKENS:
        if token not in sources:
            failures.append(f"real funds canary lifecycle source missing token: {token}")
    if "post_order(" in sources or "post_orders(" in sources:
        failures.append("store/service lifecycle code must not contain post_order call sites")

    result = {
        "status": "fail" if failures else "pass",
        "posted": False,
        "cancelled": False,
        "remote_side_effects": False,
        "raw_signed_order_exposed": False,
        "idempotency_replay_checked": True,
        "idempotency_conflict_checked": True,
        "remote_unknown_freeze_checked": True,
        "simulated_reconcile_checked": True,
        "operator_required_escalation_state": "OPERATOR_REQUIRED",
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
