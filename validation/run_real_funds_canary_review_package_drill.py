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
DECISION_VALIDATOR = ROOT / "validation" / "validate_controlled_canary_release_decision.py"
EXTERNAL_REFERENCES_VALIDATOR = ROOT / "validation" / "validate_controlled_canary_external_references.py"
BLOCKED_REHEARSAL = ROOT / "validation" / "run_real_funds_canary_blocked_rehearsal_package.py"
EXTERNAL_REFERENCES_EXAMPLE = ROOT / "config" / "controlled-canary.external-references.example.json"
EXTERNAL_REFERENCES_TEMPLATE = ROOT / "config" / "controlled-canary.external-references.template.json"
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
        "release-decision.json",
        "external-references.json",
        "default no-go",
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

    validator = subprocess.run(
        ["python", str(DECISION_VALIDATOR)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if validator.returncode != 0:
        failures.append(f"controlled canary release-decision validation failed: {validator.stderr.strip() or validator.stdout.strip()}")
    references_validator = subprocess.run(
        ["python", str(EXTERNAL_REFERENCES_VALIDATOR)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if references_validator.returncode != 0:
        failures.append(
            f"controlled canary external-reference validation failed: {references_validator.stderr.strip() or references_validator.stdout.strip()}"
        )
    blocked_rehearsal = subprocess.run(
        ["python", str(BLOCKED_REHEARSAL)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if blocked_rehearsal.returncode != 0:
        failures.append(
            f"blocked real-funds canary rehearsal package failed: {blocked_rehearsal.stderr.strip() or blocked_rehearsal.stdout.strip()}"
        )
    concrete_references_validator = subprocess.run(
        ["python", str(EXTERNAL_REFERENCES_VALIDATOR), "--file", str(EXTERNAL_REFERENCES_EXAMPLE)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if concrete_references_validator.returncode != 0:
        failures.append(
            f"controlled canary concrete external-reference validation failed: {concrete_references_validator.stderr.strip() or concrete_references_validator.stdout.strip()}"
        )
    placeholder_references_validator = subprocess.run(
        ["python", str(EXTERNAL_REFERENCES_VALIDATOR), "--file", str(EXTERNAL_REFERENCES_TEMPLATE)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if placeholder_references_validator.returncode == 0:
        failures.append("controlled canary external-reference file mode must reject unresolved placeholders")
    placeholder_references_validator_allowed = subprocess.run(
        [
            "python",
            str(EXTERNAL_REFERENCES_VALIDATOR),
            "--file",
            str(EXTERNAL_REFERENCES_TEMPLATE),
            "--allow-placeholders",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if placeholder_references_validator_allowed.returncode != 0:
        failures.append(
            f"controlled canary placeholder external-reference validation failed with allow-placeholders: {placeholder_references_validator_allowed.stderr.strip() or placeholder_references_validator_allowed.stdout.strip()}"
        )

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
        for name in ["approval.json", "external-references.json", "release-decision.json", "review.json", "README.md"]:
            if not (output_dir / name).exists():
                failures.append(f"review package missing {name}")
        if (output_dir / "review.json").exists():
            review = json.loads((output_dir / "review.json").read_text())
            if review.get("status") != "review_package_only_not_armed_approval":
                failures.append("review package must not claim armed approval")
            if review.get("live_submit_allowed") is not False:
                failures.append("review package must keep live submit disabled")
            if review.get("live_cancel_allowed") is not False:
                failures.append("review package must keep live cancel disabled")
            if review.get("real_funds_canary_authorized") is not False:
                failures.append("review package must not authorize real-funds canary")
            if review.get("remote_side_effects") is not False:
                failures.append("review package must be no-remote-side-effect")
            if review.get("secrets_included") is not False:
                failures.append("review package must not include secrets")
        if (output_dir / "release-decision.json").exists():
            decision = json.loads((output_dir / "release-decision.json").read_text())
            if decision.get("decision") != "no_go":
                failures.append("review package release decision must default to no_go")
            for key in [
                "root_ci_run_id",
                "hermes_ci_run_id",
                "execution_engine_ci_run_id",
                "credentialed_sdk_run_id",
            ]:
                if not decision.get("github_evidence", {}).get(key):
                    failures.append(f"review package release decision must bind GitHub evidence {key}")
            for key in [
                "live_submit_authorized",
                "live_cancel_authorized",
                "real_funds_canary_authorized",
                "remote_side_effects_authorized",
            ]:
                if decision.get(key) is not False:
                    failures.append(f"review package release decision must keep {key}=false")
        if (output_dir / "external-references.json").exists():
            references = json.loads((output_dir / "external-references.json").read_text())
            if references.get("references_only_no_secret_values") is not True:
                failures.append("review package external references must be reference-only")
            for key in [
                "live_submit_allowed",
                "live_cancel_allowed",
                "real_funds_canary_authorized",
                "remote_side_effects",
                "production_ready_claimed",
            ]:
                if references.get(key) is not False:
                    failures.append(f"review package external references must keep {key}=false")

    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp) / "review-with-concrete-references"
        completed = subprocess.run(
            [
                "python",
                str(SCRIPT),
                "--output-dir",
                str(output_dir),
                "--external-references-file",
                str(EXTERNAL_REFERENCES_EXAMPLE),
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0:
            failures.append(f"review package script failed with concrete external references: {completed.stderr.strip()}")
        if (output_dir / "review.json").exists():
            review = json.loads((output_dir / "review.json").read_text())
            if review.get("external_references_placeholders_remaining"):
                failures.append("review package with concrete external references must not keep placeholders")
            if review.get("live_submit_allowed") is not False:
                failures.append("review package with concrete external references must still keep live submit disabled")

    result = {
        "status": "fail" if failures else "pass",
        "review_package_generated": not failures,
        "armed_approval_created": False,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "secrets_included": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
