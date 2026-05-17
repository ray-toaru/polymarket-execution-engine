#!/usr/bin/env python3
"""Guard productionization governance without claiming production readiness."""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNBOOK = ROOT / "docs" / "PRODUCTIONIZATION_RUNBOOK.md"
CONTROLS_MATRIX = ROOT / "docs" / "PRODUCTION_CONTROLS_MATRIX.md"
HARDENING_SPEC = ROOT / "docs" / "PRODUCTION_HARDENING_SPEC.md"
RELEASE_MANIFEST = ROOT / "release" / "manifest.json"
EVIDENCE_GUARD = ROOT / "validation" / "check_v0_23_evidence_manifest.py"
GATE = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST_WRITER = ROOT / "validation" / "write_v0_23_evidence_manifest.py"

RUNBOOK_TOKENS = [
    "Secret manager",
    "KMS",
    "HSM",
    "Production config profile",
    "Deployment runbook",
    "Rollback runbook",
    "Incident drill",
    "Alerting and dashboard",
    "SLO and error budget",
    "Audit export and retention policy",
    "Account and market risk limits",
    "Dependency update policy",
    "SDK upstream breakage playbook",
    "production-ready is forbidden",
]

CONTROLS_TOKENS = [
    "Secret manager / KMS / HSM",
    "Production config profile",
    "Deployment runbook",
    "Rollback runbook",
    "Incident drill",
    "Alerting and dashboard",
    "SLO / error budget",
    "Audit export / retention policy",
    "Account risk limits",
    "Market risk limits",
    "Dependency update policy",
    "SDK upstream breakage playbook",
    "artifact",
    "non-production",
]

HARDENING_TOKENS = [
    "Secret Custody",
    "secret manager",
    "KMS",
    "HSM",
    "Deployment And Rollback",
    "artifact SHA-256",
    "config kill switch",
    "Observability",
    "runtime worker health",
    "remote unknown freeze",
    "SLO And Error Budget",
    "Audit Export And Retention",
    "Risk Limits",
    "Account whitelist",
    "Market whitelist",
    "Dependency And SDK Breakage",
    "sign-only regression evidence",
]


def main() -> int:
    failures: list[str] = []
    runbook = RUNBOOK.read_text()
    for token in RUNBOOK_TOKENS:
        if token not in runbook:
            failures.append(f"production runbook missing {token}")

    controls = CONTROLS_MATRIX.read_text()
    for token in CONTROLS_TOKENS:
        if token not in controls:
            failures.append(f"production controls matrix missing {token}")

    hardening = HARDENING_SPEC.read_text()
    for token in HARDENING_TOKENS:
        if token not in hardening:
            failures.append(f"production hardening spec missing {token}")

    release = json.loads(RELEASE_MANIFEST.read_text())
    status = str(release.get("status", "")).lower()
    if "production-ready" in status or "production_ready" in status:
        failures.append("release manifest must not claim production-ready")
    if "production-readiness-not-claimed" not in release.get("remaining_blockers", []):
        failures.append("release manifest must preserve production-readiness-not-claimed blocker")

    evidence_guard = EVIDENCE_GUARD.read_text()
    for token in ["validated_release=true", "artifact.sha256", "non-pass evidence sections"]:
        if token not in evidence_guard:
            failures.append(f"evidence guard missing anti-overclaim token: {token}")

    gates = GATE.read_text()
    manifest_writer = MANIFEST_WRITER.read_text()
    if "36-production-readiness-guard.log" not in gates:
        failures.append("run_v0_23_gates.sh must emit production readiness guard log")
    if '"productionization_validation"' not in manifest_writer:
        failures.append("evidence manifest must include productionization_validation")
    if "36-production-readiness-guard.log" not in manifest_writer:
        failures.append("evidence manifest must capture production readiness guard log")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("production readiness guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
