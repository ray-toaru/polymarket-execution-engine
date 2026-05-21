#!/usr/bin/env python3
"""Prove a complete review package still blocks armed canary under no-go."""
from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REVIEW_SCRIPT = ROOT / "validation" / "prepare_real_funds_canary_review.py"
EXTERNAL_REFERENCES_EXAMPLE = ROOT / "config" / "controlled-canary.external-references.example.json"
ADAPTER_MANIFEST = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.toml"


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def main() -> int:
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp) / "review"
        completed = subprocess.run(
            [
                "python",
                str(REVIEW_SCRIPT),
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
            failures.append(f"review package generation failed: {completed.stderr.strip()}")
        review = load(output_dir / "review.json") if (output_dir / "review.json").exists() else {}
        approval = load(output_dir / "approval.json") if (output_dir / "approval.json").exists() else {}
        if review.get("live_submit_allowed") is not False:
            failures.append("review package must keep live_submit_allowed=false")
        if review.get("real_funds_canary_authorized") is not False:
            failures.append("review package must keep real_funds_canary_authorized=false")

        no_go_decision = {
            "decision_id": "blocked-rehearsal-no-go",
            "scope": "REAL_FUNDS_CANARY",
            "expires_at": "2099-01-01T00:00:00Z",
            "artifact_sha256": approval.get("artifact_sha256"),
            "evidence_manifest_sha256": approval.get("evidence_manifest_sha256"),
            "allow_real_funds_canary": False,
            "reviewed_release_decision_present": True,
            "operator_identity_ref": "local-blocked-rehearsal-operator",
        }
        decision_path = output_dir / "adapter-release-decision.no-go.json"
        decision_path.write_text(json.dumps(no_go_decision, indent=2, sort_keys=True) + "\n")

        command = [
            "cargo",
            "run",
            "--manifest-path",
            str(ADAPTER_MANIFEST.relative_to(ROOT)),
            "--features",
            "live-submit",
            "--bin",
            "pmx-real-funds-canary",
            "--",
            "--armed",
            "--approval-file",
            str(output_dir / "approval.json"),
            "--release-decision-file",
            str(decision_path),
            "--artifact-sha256",
            str(approval.get("artifact_sha256")),
            "--evidence-manifest-sha256",
            str(approval.get("evidence_manifest_sha256")),
            "--idempotency-key",
            "blocked-rehearsal-idempotency",
            "--account-id",
            approval.get("account_id", "acct-canary"),
            "--execution-id",
            "blocked-rehearsal-execution",
            "--plan-hash",
            "blocked-rehearsal-plan-hash",
            "--allow-live-submit-config",
            "--allow-real-funds-canary-config",
        ]
        rehearsal = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        expected_error = "real-funds canary not allowed by release decision"
        if rehearsal.returncode == 0:
            failures.append("armed no-go rehearsal must fail before posting")
        if expected_error not in rehearsal.stderr:
            failures.append("armed no-go rehearsal missing release-decision block reason")
        forbidden = ["posted\": true", "remote_side_effects\": true", "raw_signed_order_exposed\": true"]
        combined_output = rehearsal.stdout + "\n" + rehearsal.stderr
        for token in forbidden:
            if token in combined_output:
                failures.append(f"armed no-go rehearsal emitted forbidden token: {token}")

    result = {
        "status": "fail" if failures else "pass",
        "rehearsal": "blocked_real_funds_canary_armed_no_go",
        "armed_requested": True,
        "allow_live_submit_config": True,
        "allow_real_funds_canary_config": True,
        "expected_exit_code": 1,
        "observed_exit_code": rehearsal.returncode if "rehearsal" in locals() else None,
        "blocked_at": "release_decision_gate",
        "blocked_reason": "real-funds canary not allowed by release decision",
        "posted": False,
        "cancelled": False,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "raw_signed_order_exposed": False,
        "secrets_included": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
