#!/usr/bin/env python3
"""Prove a complete review package still blocks armed canary under no-go."""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
REVIEW_SCRIPT = ROOT / "validation" / "prepare_real_funds_canary_review.py"
EXTERNAL_REFERENCES_EXAMPLE = ROOT / "config" / "controlled-canary.external-references.example.json"
ADAPTER_MANIFEST = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.toml"
EXAMPLE_REVIEW_ARTIFACT_SHA256 = "c0c22c91541d48c508a588b06a2fa5d7051bc6c8e29df626de67a59cc96c24e6"


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def resolve_input_path(path: Path) -> Path:
    if path.is_absolute() or path.exists():
        return path
    integration_path = INTEGRATION_ROOT / path
    if integration_path.exists():
        return integration_path
    return path


def adapter_source_release() -> str:
    for line in ADAPTER_MANIFEST.read_text().splitlines():
        stripped = line.strip()
        if stripped.startswith("version = "):
            return "v" + stripped.split("=", 1)[1].strip().strip('"')
    raise SystemExit(f"could not read adapter package version from {ADAPTER_MANIFEST}")


def run_rehearsal(output_dir: Path, args: argparse.Namespace) -> tuple[list[str], int | None]:
    failures: list[str] = []
    external_references_file = resolve_input_path(args.external_references_file)
    review_command = [
        "python",
        str(REVIEW_SCRIPT),
        "--output-dir",
        str(output_dir),
        "--external-references-file",
        str(external_references_file),
        "--root-ci-run-id",
        args.root_ci_run_id,
        "--hermes-ci-run-id",
        args.hermes_ci_run_id,
        "--execution-engine-ci-run-id",
        args.execution_engine_ci_run_id,
        "--credentialed-sdk-run-id",
        args.credentialed_sdk_run_id,
    ]
    if args.artifact_sha256:
        review_command.extend(["--artifact-sha256", args.artifact_sha256])
    if args.evidence_manifest_sha256:
        review_command.extend(["--evidence-manifest-sha256", args.evidence_manifest_sha256])
    completed = subprocess.run(
        review_command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        failures.append(f"review package generation failed: {completed.stderr.strip()}")
        return failures, None
    review = load(output_dir / "review.json") if (output_dir / "review.json").exists() else {}
    approval = load(output_dir / "approval.json") if (output_dir / "approval.json").exists() else {}
    release_decision = load(output_dir / "release-decision.json") if (output_dir / "release-decision.json").exists() else {}
    if review.get("live_submit_allowed") is not False:
        failures.append("review package must keep live_submit_allowed=false")
    if review.get("real_funds_canary_authorized") is not False:
        failures.append("review package must keep real_funds_canary_authorized=false")

    no_go_decision = {
        **release_decision,
        "schema_version": 1,
        "decision_id": "blocked-rehearsal-no-go",
        "status": "reviewed_no_go_blocked_rehearsal",
        "source_release": adapter_source_release(),
        "decision": "no_go",
        "decision_reason": "Blocked rehearsal must prove an armed command fails before any remote side effect.",
        "scope": "REAL_FUNDS_CANARY",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "expires_at": "2099-01-01T00:00:00Z",
        "artifact_sha256": approval.get("artifact_sha256"),
        "evidence_manifest_sha256": approval.get("evidence_manifest_sha256"),
        "market_candidate_sha256": approval.get("market_candidate_sha256"),
        "github_evidence": release_decision.get("github_evidence", {}),
        "external_references": release_decision.get("external_references", {}),
        "risk_limits": release_decision.get("risk_limits", {}),
        "required_review_signals": release_decision.get("required_review_signals", {}),
        "live_submit_authorized": False,
        "live_cancel_authorized": False,
        "production_deployment_authorized": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects_authorized": False,
        "allow_real_funds_canary": False,
        "reviewed_release_decision_present": True,
        "operator_identity_ref": approval.get("operator_identity_ref"),
        "secrets_included": False,
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
        args.idempotency_key,
        "--account-id",
        approval.get("account_id", "acct-canary"),
        "--execution-id",
        args.execution_id,
        "--plan-hash",
        args.plan_hash,
        "--market-file",
        str(output_dir / "candidate-market.json"),
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
    (output_dir / "blocked-rehearsal.stdout").write_text(rehearsal.stdout)
    (output_dir / "blocked-rehearsal.stderr").write_text(rehearsal.stderr)
    (output_dir / "blocked-rehearsal.exit-code").write_text(f"{rehearsal.returncode}\n")

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
    return failures, rehearsal.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--external-references-file", type=Path, default=EXTERNAL_REFERENCES_EXAMPLE)
    parser.add_argument("--artifact-sha256", default=EXAMPLE_REVIEW_ARTIFACT_SHA256)
    parser.add_argument("--evidence-manifest-sha256")
    parser.add_argument("--root-ci-run-id", default="26268697168")
    parser.add_argument("--hermes-ci-run-id", default="26267887116")
    parser.add_argument("--execution-engine-ci-run-id", default="26268276210")
    parser.add_argument("--credentialed-sdk-run-id", default="local-current-gates-20260521")
    parser.add_argument("--idempotency-key", default="blocked-rehearsal-idempotency")
    parser.add_argument("--execution-id", default="blocked-rehearsal-execution")
    parser.add_argument("--plan-hash", default="blocked-rehearsal-plan-hash")
    args = parser.parse_args()

    if args.output_dir:
        output_dir = args.output_dir if args.output_dir.is_absolute() else INTEGRATION_ROOT / args.output_dir
        output_dir.mkdir(parents=True, exist_ok=True)
        failures, observed_exit_code = run_rehearsal(output_dir, args)
    else:
        with tempfile.TemporaryDirectory() as tmp:
            output_dir = Path(tmp) / "review"
            failures, observed_exit_code = run_rehearsal(output_dir, args)

    result = {
        "status": "fail" if failures else "pass",
        "rehearsal": "blocked_real_funds_canary_armed_no_go",
        "armed_requested": True,
        "allow_live_submit_config": True,
        "allow_real_funds_canary_config": True,
        "expected_exit_code": 1,
        "observed_exit_code": observed_exit_code,
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
        "output_dir": str(output_dir),
        "stdout_log": str(output_dir / "blocked-rehearsal.stdout"),
        "stderr_log": str(output_dir / "blocked-rehearsal.stderr"),
        "failures": failures,
    }
    if args.output_dir:
        (output_dir / "blocked-rehearsal.report.json").write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n"
        )
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
