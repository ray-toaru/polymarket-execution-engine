#!/usr/bin/env python3
"""Validate local real-funds canary review package generation."""
from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "validation" / "prepare_real_funds_canary_review.py"
DOC = ROOT / "docs" / "REAL_FUNDS_CANARY_OPERATIONS_READINESS.md"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"


def main() -> int:
    failures: list[str] = []
    require_current_gate_log(
        "68-real-funds-canary-review-package.log",
        "real funds canary review package drill",
        failures,
    )

    doc = DOC.read_text() if DOC.exists() else ""
    for token in [
        "reviewed release decision JSON",
        "external secret provider reference",
        "external alert routing reference",
        "rollback runbook",
        "canary retry policy",
        "live_submit_allowed=false",
        "remote_side_effects=false",
        "secrets_included=false",
    ]:
        if token not in doc:
            failures.append(f"operations readiness doc missing token: {token}")

    writer = MANIFEST_WRITER.read_text()
    if '"real_funds_canary_review_package_validation"' not in writer:
        failures.append("evidence manifest must include real_funds_canary_review_package_validation")
    if "68-real-funds-canary-review-package.log" not in writer:
        failures.append("evidence manifest must capture real-funds canary review package log")

    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp) / "review"
        completed = subprocess.run(
            ["python", str(SCRIPT), "--output-dir", str(output_dir)],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0:
            failures.append(f"review package script failed: {completed.stderr.strip()}")
        for name in ["approval.json", "review.json", "README.md"]:
            if not (output_dir / name).exists():
                failures.append(f"review package missing {name}")
        if (output_dir / "review.json").exists():
            review = json.loads((output_dir / "review.json").read_text())
            if review.get("status") != "review_package_only_not_armed_approval":
                failures.append("review package must not claim armed approval")
            if review.get("live_submit_allowed") is not False:
                failures.append("review package must keep live submit disabled")
            if review.get("remote_side_effects") is not False:
                failures.append("review package must be no-remote-side-effect")
            if review.get("secrets_included") is not False:
                failures.append("review package must not include secrets")

    result = {
        "status": "fail" if failures else "pass",
        "review_package_generated": not failures,
        "armed_approval_created": False,
        "live_submit_allowed": False,
        "remote_side_effects": False,
        "secrets_included": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
