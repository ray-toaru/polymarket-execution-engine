#!/usr/bin/env python3
"""Prepare or run the privileged armed reviewed-go canary invocation."""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
BASE_SCRIPT = INTEGRATION_ROOT / "scripts" / "run_reviewed_go_canary.py"


def load_base():
    spec = importlib.util.spec_from_file_location("run_reviewed_go_canary_base", BASE_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--env-file", required=True, type=Path)
    parser.add_argument("--secrets-env-file", type=Path)
    parser.add_argument("--daily-used-notional-usd", default="0")
    parser.add_argument("--idempotency-key")
    parser.add_argument("--execution-id")
    parser.add_argument("--plan-hash")
    parser.add_argument("--report-file", type=Path)
    parser.add_argument("--approval-consumed-marker", type=Path)
    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute the privileged armed cargo command. Without this flag the script only prints the invocation plan.",
    )
    return parser.parse_args()


def build_armed_invocation(
    *,
    package_dir: Path,
    env_file: Path,
    secrets_env_file: Path | None,
    daily_used_notional_usd: str,
    idempotency_key: str | None,
    execution_id: str | None,
    plan_hash: str | None,
    report_file: Path | None,
    approval_consumed_marker: Path | None,
) -> dict[str, object]:
    base = load_base()
    package_dir = base.resolve(package_dir)
    env_file = base.resolve(env_file)
    secrets_env_file = base.resolve(secrets_env_file) if secrets_env_file else None
    release_decision_file = base.require_file(package_dir / "release-decision.json", "release decision")
    approval_file = base.require_file(package_dir / "approval.json", "approval")
    pipeline = base.load_module(base.PIPELINE_SCRIPT, "run_controlled_canary_pipeline")

    decision_summary = pipeline.validate_reviewed_go_decision_file(release_decision_file)
    approval = base.validate_approval(approval_file)
    market_file = base.require_file(package_dir / "candidate-market.json", "candidate market")
    runtime_truth_file = base.require_file(package_dir / "runtime-truth.json", "runtime truth")
    env_check = base.load_module(base.ENV_CHECK_SCRIPT, "check_active_profile_consistency")
    runtime_truth_summary = pipeline.validate_runtime_truth_file(
        runtime_truth_file,
        expected_account_id=approval["account_id"],
    )
    pipeline.validate_candidate_file(market_file)
    env_summary = env_check.evaluate_env_file(
        env_file,
        expected_account_id=approval["account_id"],
        secrets_env_file=secrets_env_file,
    )
    base.require_runtime_truth_gate_alignment(runtime_truth_summary)
    gate_snapshot, gate_evidence_refs = base.require_approval_runtime_gate_alignment(
        approval, runtime_truth_summary
    )
    if approval["artifact_sha256"] != runtime_truth_summary["artifact_sha256"]:
        raise SystemExit("approval artifact_sha256 does not match runtime truth artifact_sha256")
    if approval["workspace_manifest_sha256"] != runtime_truth_summary["workspace_manifest_sha256"]:
        raise SystemExit("approval workspace_manifest_sha256 does not match runtime truth")
    if approval["archived_manifest_sha256"] != runtime_truth_summary["archived_manifest_sha256"]:
        raise SystemExit("approval archived_manifest_sha256 does not match runtime truth")

    resolved_plan_hash = plan_hash or base.plan_hash_from_package(approval)
    invocation_hash = base.invocation_hash_from_package(
        approval,
        mode="armed",
        runtime_truth_sha256=runtime_truth_summary["sha256"],
        active_profile_ref=env_summary["active_profile_ref"],
        plan_hash=resolved_plan_hash,
        daily_used_notional_usd=daily_used_notional_usd,
    )
    resolved_idempotency_key = idempotency_key or f"canary-{invocation_hash}-armed"
    resolved_execution_id = execution_id or f"exec-{invocation_hash}"
    resolved_report_file = base.resolve(report_file) if report_file else (package_dir / "post-canary-report.json")
    resolved_consumed_marker = (
        base.resolve(approval_consumed_marker)
        if approval_consumed_marker
        else base.default_marker_path(package_dir)
    )
    command = [
        "cargo",
        "run",
        "--manifest-path",
        str(base.ADAPTER_MANIFEST),
        "--features",
        "live-submit",
        "--bin",
        "pmx-real-funds-canary-armed",
        "--",
        "--env-file",
        str(env_file),
        "--approval-file",
        str(approval_file),
        "--release-decision-file",
        str(release_decision_file),
        "--runtime-truth-file",
        str(runtime_truth_file),
        "--runtime-truth-condition-id",
        approval["condition_id"],
        "--market-file",
        str(market_file),
        "--artifact-sha256",
        approval["artifact_sha256"],
        "--evidence-manifest-sha256",
        approval["evidence_manifest_sha256"],
        "--idempotency-key",
        resolved_idempotency_key,
        "--account-id",
        approval["account_id"],
        "--execution-id",
        resolved_execution_id,
        "--plan-hash",
        resolved_plan_hash,
        "--daily-used-notional-usd",
        daily_used_notional_usd,
        "--approval-consumed-marker",
        str(resolved_consumed_marker),
        "--report-file",
        str(resolved_report_file),
    ]
    invocation = {
        "status": "ready",
        "mode": "armed",
        "package_dir": str(package_dir),
        "env_file": str(env_file),
        "account_id": approval["account_id"],
        "condition_id": approval["condition_id"],
        "active_profile_ref": env_summary["active_profile_ref"],
        "approval_hash": approval["approval_hash"],
        "decision_id": decision_summary["decision_id"],
        "runtime_truth_sha256": runtime_truth_summary["sha256"],
        "invocation_hash": invocation_hash,
        "runtime_gate_snapshot": gate_snapshot,
        "runtime_gate_evidence_refs": gate_evidence_refs,
        "command": command,
        "required_gate_env_vars": [],
        "missing_gate_env_vars": [],
        "includes_live_config_overrides": False,
        "requires_explicit_live_config_overrides": False,
        "report_file": str(resolved_report_file),
        "approval_consumed_marker": str(resolved_consumed_marker),
    }
    invocation["wrapper"] = "run_reviewed_go_canary_armed.py"
    invocation["armed_wrapper"] = True
    return invocation


def main() -> int:
    args = parse_args()
    base = load_base()
    invocation = build_armed_invocation(
        package_dir=args.package_dir,
        env_file=args.env_file,
        secrets_env_file=args.secrets_env_file,
        daily_used_notional_usd=args.daily_used_notional_usd,
        idempotency_key=args.idempotency_key,
        execution_id=args.execution_id,
        plan_hash=args.plan_hash,
        report_file=args.report_file,
        approval_consumed_marker=args.approval_consumed_marker,
    )
    if not args.run:
        print(json.dumps(invocation, indent=2, sort_keys=True))
        return 0

    completed = subprocess.run(
        invocation["command"],
        cwd=INTEGRATION_ROOT,
        text=True,
        check=False,
    )
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
