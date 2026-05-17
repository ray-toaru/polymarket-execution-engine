#!/usr/bin/env python3
"""Guard v0.23 evidence manifests against accidental release overclaiming.

The template lives under validation/templates so evidence/current remains the single
canonical evidence location.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "validation" / "templates" / "evidence_manifest.template.json"
CURRENT = ROOT / "evidence" / "current" / "manifest.json"
REQUIRED_SECTIONS = [
    "local_static_validation",
    "rust_workspace_validation",
    "postgres_validation",
    "sdk_adapter_validation",
    "credentialed_non_trading_validation",
]
VALID_STATUSES = {"pending", "pass", "fail", "skipped", "not_run"}


def fail(message: str) -> int:
    print(f"FAIL: {message}")
    return 1


def validate(path: Path) -> int:
    data = json.loads(path.read_text())
    if data.get("version") != "0.23.0":
        return fail("manifest version must be 0.23.0")
    if data.get("artifact_kind") not in {"source_candidate", "validated_release"}:
        return fail("artifact_kind must be source_candidate or validated_release")
    if data.get("canonical_evidence_dir") != "polymarket-execution-engine/evidence/current":
        return fail("canonical_evidence_dir must point at evidence/current")
    artifact = data.get("artifact")
    if not isinstance(artifact, dict):
        return fail("missing artifact block")
    for section in REQUIRED_SECTIONS:
        block = data.get(section)
        if not isinstance(block, dict):
            return fail(f"missing evidence section: {section}")
        status = block.get("status")
        if status not in VALID_STATUSES:
            return fail(f"invalid status for {section}: {status}")
        required_logs = block.get("required_logs")
        logs = block.get("logs")
        if required_logs is not None and (not isinstance(required_logs, list) or not all(isinstance(item, str) and item for item in required_logs)):
            return fail(f"{section}.required_logs must be a non-empty string list when present")
        if logs is not None and not isinstance(logs, list):
            return fail(f"{section}.logs must be a list when present")
    decision = data.get("release_decision")
    if not isinstance(decision, dict):
        return fail("missing release_decision")
    if decision.get("validated_release") is True:
        non_pass = [section for section in REQUIRED_SECTIONS if data[section].get("status") != "pass"]
        if non_pass:
            return fail(f"validated_release=true with non-pass evidence sections: {non_pass}")
        if data.get("artifact_kind") != "validated_release":
            return fail("validated_release=true requires artifact_kind=validated_release")
        if not artifact.get("sha256"):
            return fail("validated_release=true requires artifact.sha256")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) > 1:
        paths = [Path(arg) for arg in argv[1:]]
    else:
        paths = [TEMPLATE]
        if CURRENT.exists():
            paths.append(CURRENT)
    for path in paths:
        if not path.exists():
            return fail(f"manifest not found: {path}")
        rc = validate(path)
        if rc != 0:
            return rc
    print("v0.23 evidence manifest guard passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
