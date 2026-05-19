#!/usr/bin/env python3
"""Guard release metadata against accidental production/live promotion claims."""
from __future__ import annotations

import json
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DOC = ROOT / "docs" / "PRODUCTION_RELEASE_DECISION_GUARD.md"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
ENGINE_RELEASE_MANIFEST = ROOT / "release" / "manifest.json"
ROOT_RELEASE_DECISION = INTEGRATION_ROOT / "RELEASE_DECISION.md"
CURRENT_MANIFEST = ROOT / "evidence" / "current" / "manifest.json"


def main() -> int:
    failures: list[str] = []
    required = [
        "release_status_not_production_ready",
        "release_status_not_live_ready",
        "validated_release_false",
        "production_ready_false",
        "live_trading_ready_false",
        "production_blocker_present",
        "live_blocker_present",
        "artifact_kind_source_candidate",
        "no_production_promotion_without_review",
        "production_ready_claimed = false",
        "live_ready_claimed = false",
        "validated_release = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production release decision guard document missing")
    for token in required:
        if token not in doc:
            failures.append(f"production release decision guard document missing token: {token}")

    manifest_writer = MANIFEST_WRITER.read_text()
    require_current_gate_log("57-production-release-decision-guard.log", "production release decision guard", failures)
    if '"production_release_decision_guard_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_release_decision_guard_validation")

    release = json.loads(ENGINE_RELEASE_MANIFEST.read_text())
    status = str(release.get("status", "")).lower()
    blockers = set(release.get("remaining_blockers", []))
    if "production-ready" in status or "live-ready" in status:
        failures.append("engine release manifest status must not claim production/live readiness")
    if "production-readiness-not-claimed" not in blockers:
        failures.append("engine release manifest must preserve production blocker")
    if "live-submit-live-cancel-production-deployment-remain-blocked" not in blockers:
        failures.append("engine release manifest must preserve live/deployment blocker")

    if CURRENT_MANIFEST.exists():
        current = json.loads(CURRENT_MANIFEST.read_text())
        decision = current.get("release_decision", {})
        if current.get("artifact_kind") != "source_candidate":
            failures.append("current evidence artifact_kind must remain source_candidate")
        if decision.get("validated_release") is not False:
            failures.append("current evidence validated_release must remain false")
        if decision.get("production_ready") is not False:
            failures.append("current evidence production_ready must remain false")
        if decision.get("live_trading_ready") is not False:
            failures.append("current evidence live_trading_ready must remain false")

    root_decision = ROOT_RELEASE_DECISION.read_text() if ROOT_RELEASE_DECISION.exists() else ""
    for token in ["Do not claim production readiness", "Do not claim live-trading readiness", "not production-ready"]:
        if token not in root_decision:
            failures.append(f"root release decision missing truthfulness token: {token}")

    result = {
        "status": "fail" if failures else "pass",
        "release_status_not_production_ready": True,
        "release_status_not_live_ready": True,
        "validated_release": False,
        "production_ready_claimed": False,
        "live_ready_claimed": False,
        "remote_side_effects": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
