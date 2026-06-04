#!/usr/bin/env python3
"""Prepare or run the reviewed-go preflight invocation from a fresh package."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
PIPELINE_SCRIPT = INTEGRATION_ROOT / "scripts" / "run_controlled_canary_pipeline.py"
ENV_CHECK_SCRIPT = (
    ROOT / "validation" / "check_active_profile_consistency.py"
)
ADAPTER_MANIFEST = (
    ROOT
    / "adapters"
    / "pmx-official-sdk-adapter"
    / "Cargo.toml"
)
RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS = {
    "PMX_KILL_SWITCH_OPEN": "kill_switch_open",
    "PMX_RUNTIME_WORKER_HEALTHY": "runtime_worker_healthy",
    "PMX_GEOBLOCK_ALLOWED": "geoblock_allowed",
    "PMX_REPOSITORY_RESERVATION_EXISTS": "repository_reservation_exists",
    "PMX_IDEMPOTENCY_KEY_WRITTEN": "idempotency_key_written",
    "PMX_RECONCILE_WORKER_HEALTHY": "reconcile_worker_healthy",
    "PMX_CANCEL_ONLY_FALLBACK_READY": "cancel_only_fallback_ready",
    "PMX_BALANCE_ALLOWANCE_CHECKED": "balance_allowance_checked",
}


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def resolve(path: Path) -> Path:
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def require_file(path: Path, label: str) -> Path:
    if not path.is_file():
        raise SystemExit(f"{label} missing: {path}")
    return path


def require_text(data: dict[str, Any], field: str) -> str:
    value = data.get(field)
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"{field} is required")
    return value.strip()


def validate_approval(path: Path) -> dict[str, Any]:
    data = load_json(path)
    if data.get("scope") != "REAL_FUNDS_CANARY":
        raise SystemExit("approval scope must be REAL_FUNDS_CANARY")
    if data.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
        raise SystemExit("approval execution_style must be GTC_LIMIT_POST_ONLY_CANCEL")
    for field in [
        "approval_hash",
        "artifact_sha256",
        "evidence_manifest_sha256",
        "workspace_manifest_sha256",
        "archived_manifest_sha256",
        "market_candidate_sha256",
        "operator_identity_sha256",
    ]:
        require_text(data, field)
    require_text(data, "account_id")
    require_text(data, "condition_id")
    gate_snapshot = data.get("runtime_gate_snapshot")
    if not isinstance(gate_snapshot, dict):
        raise SystemExit("approval runtime_gate_snapshot must be an object")
    for report_field in [
        "preconditions_live_submit_would_pass",
        "preconditions_real_funds_canary_would_pass",
        *RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS.values(),
    ]:
        if gate_snapshot.get(report_field) is not True:
            raise SystemExit(f"approval runtime_gate_snapshot.{report_field} must be true")
    gate_evidence_refs = data.get("runtime_gate_evidence_refs")
    if not isinstance(gate_evidence_refs, dict):
        raise SystemExit("approval runtime_gate_evidence_refs must be an object")
    for report_field in RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS.values():
        evidence_ref = gate_evidence_refs.get(report_field)
        if not isinstance(evidence_ref, str) or not evidence_ref.strip():
            raise SystemExit(
                f"approval runtime_gate_evidence_refs.{report_field} must be a non-empty string"
            )
    return data


def plan_hash_from_package(approval: dict[str, Any]) -> str:
    raw = "|".join(
        [
            approval["approval_hash"],
            approval["artifact_sha256"],
            approval["evidence_manifest_sha256"],
            approval["market_candidate_sha256"],
        ]
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def invocation_hash_from_package(
    approval: dict[str, Any],
    *,
    mode: str,
    runtime_truth_sha256: str,
    active_profile_ref: str,
    plan_hash: str,
    daily_used_notional_usd: str,
) -> str:
    raw = "|".join(
        [
            mode,
            approval["approval_hash"],
            approval["artifact_sha256"],
            approval["workspace_manifest_sha256"],
            approval["archived_manifest_sha256"],
            approval["market_candidate_sha256"],
            approval["account_id"],
            approval["condition_id"],
            active_profile_ref,
            runtime_truth_sha256,
            plan_hash,
            daily_used_notional_usd,
        ]
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def timestamp_tag() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def default_marker_path(package_dir: Path) -> Path:
    return package_dir / f"approval-consumed-{timestamp_tag()}.json"


def require_runtime_truth_gate_alignment(runtime_truth_summary: dict[str, Any]) -> None:
    report = runtime_truth_summary.get("preflight_report")
    if not isinstance(report, dict):
        raise SystemExit("runtime truth preflight_report must be an object")
    if report.get("live_submit_allowed") is not False:
        raise SystemExit("runtime truth preflight_report.live_submit_allowed must remain false for reviewed-go wrapper use")
    if report.get("real_funds_canary_allowed") is not False:
        raise SystemExit("runtime truth preflight_report.real_funds_canary_allowed must remain false for reviewed-go wrapper use")
    for field in ["preconditions_live_submit_would_pass", "preconditions_real_funds_canary_would_pass"]:
        if report.get(field) is not True:
            raise SystemExit(f"runtime truth preflight_report.{field} must be true for reviewed-go wrapper use")
    for report_field in RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS.values():
        if report.get(report_field) is not True:
            raise SystemExit(f"runtime truth preflight_report.{report_field} must be true")


def require_approval_runtime_gate_alignment(
    approval: dict[str, Any],
    runtime_truth_summary: dict[str, Any],
) -> tuple[dict[str, bool], dict[str, str]]:
    approval_snapshot = approval.get("runtime_gate_snapshot")
    report = runtime_truth_summary.get("preflight_report")
    if not isinstance(approval_snapshot, dict) or not isinstance(report, dict):
        raise SystemExit("approval/runtime truth gate snapshots must both be objects")
    gate_snapshot: dict[str, bool] = {}
    for report_field in [
        "preconditions_live_submit_would_pass",
        "preconditions_real_funds_canary_would_pass",
        *RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS.values(),
    ]:
        if approval_snapshot.get(report_field) is not True or report.get(report_field) is not True:
            raise SystemExit(f"approval/runtime truth gate snapshot {report_field} must be true")
        gate_snapshot[report_field] = True
    approval_evidence_refs = approval.get("runtime_gate_evidence_refs")
    runtime_evidence_refs = report.get("gate_evidence_refs")
    if not isinstance(approval_evidence_refs, dict) or not isinstance(runtime_evidence_refs, dict):
        raise SystemExit("approval/runtime truth gate evidence refs must both be objects")
    gate_evidence_refs: dict[str, str] = {}
    for report_field in RUNTIME_TRUTH_PREFLIGHT_ENV_BINDINGS.values():
        approval_ref = approval_evidence_refs.get(report_field)
        runtime_ref = runtime_evidence_refs.get(report_field)
        if not isinstance(approval_ref, str) or not approval_ref.strip():
            raise SystemExit(f"approval runtime_gate_evidence_refs.{report_field} must be a non-empty string")
        if not isinstance(runtime_ref, str) or not runtime_ref.strip():
            raise SystemExit(
                f"runtime truth preflight_report.gate_evidence_refs.{report_field} must be a non-empty string"
            )
        if approval_ref != runtime_ref:
            raise SystemExit(
                f"approval/runtime truth gate evidence ref mismatch for {report_field}"
            )
        gate_evidence_refs[report_field] = approval_ref
    return gate_snapshot, gate_evidence_refs


def build_invocation(
    *,
    package_dir: Path,
    env_file: Path,
    secrets_env_file: Path | None = None,
    mode: str,
    daily_used_notional_usd: str,
    idempotency_key: str | None,
    execution_id: str | None,
    plan_hash: str | None,
    report_file: Path | None,
    approval_consumed_marker: Path | None,
    include_live_config_overrides: bool,
) -> dict[str, Any]:
    if mode != "preflight":
        raise SystemExit(
            "run_reviewed_go_canary.py only supports preflight; use run_reviewed_go_canary_armed.py for armed invocations"
        )
    if include_live_config_overrides:
        raise SystemExit(
            "live config overrides are only valid for armed reviewed-go canary invocations"
        )
    pipeline = load_module(PIPELINE_SCRIPT, "run_controlled_canary_pipeline")
    env_check = load_module(ENV_CHECK_SCRIPT, "check_active_profile_consistency")

    release_decision_file = require_file(package_dir / "release-decision.json", "release decision")
    approval_file = require_file(package_dir / "approval.json", "approval")
    market_file = require_file(package_dir / "candidate-market.json", "candidate market")
    runtime_truth_file = require_file(package_dir / "runtime-truth.json", "runtime truth")

    decision_summary = pipeline.validate_reviewed_go_decision_file(release_decision_file)
    approval = validate_approval(approval_file)
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
    require_runtime_truth_gate_alignment(runtime_truth_summary)
    gate_snapshot, gate_evidence_refs = require_approval_runtime_gate_alignment(
        approval, runtime_truth_summary
    )

    if approval["artifact_sha256"] != runtime_truth_summary["artifact_sha256"]:
        raise SystemExit("approval artifact_sha256 does not match runtime truth artifact_sha256")
    if approval["workspace_manifest_sha256"] != runtime_truth_summary["workspace_manifest_sha256"]:
        raise SystemExit("approval workspace_manifest_sha256 does not match runtime truth")
    if approval["archived_manifest_sha256"] != runtime_truth_summary["archived_manifest_sha256"]:
        raise SystemExit("approval archived_manifest_sha256 does not match runtime truth")

    plan = plan_hash or plan_hash_from_package(approval)
    invocation_hash = invocation_hash_from_package(
        approval,
        mode="preflight",
        runtime_truth_sha256=runtime_truth_summary["sha256"],
        active_profile_ref=env_summary["active_profile_ref"],
        plan_hash=plan,
        daily_used_notional_usd=daily_used_notional_usd,
    )
    idempotency = idempotency_key or f"canary-{invocation_hash}-preflight"
    execution = execution_id or f"exec-{invocation_hash}"

    command = [
        "cargo",
        "run",
        "--manifest-path",
        str(ADAPTER_MANIFEST),
        "--features",
        "live-submit",
        "--bin",
        "pmx-real-funds-canary-preflight",
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
        idempotency,
        "--account-id",
        approval["account_id"],
        "--execution-id",
        execution,
        "--plan-hash",
        plan,
        "--daily-used-notional-usd",
        daily_used_notional_usd,
    ]

    return {
        "status": "ready",
        "mode": "preflight",
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
        "report_file": None,
        "approval_consumed_marker": None,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--env-file", required=True, type=Path)
    parser.add_argument(
        "--secrets-env-file",
        type=Path,
        help="Optional explicit runtime companion secrets env file. Secrets are not auto-discovered from the env-file path.",
    )
    parser.add_argument("--mode", choices=["preflight"], default="preflight")
    parser.add_argument("--daily-used-notional-usd", default="0")
    parser.add_argument("--idempotency-key")
    parser.add_argument("--execution-id")
    parser.add_argument("--plan-hash")
    parser.add_argument("--report-file", type=Path)
    parser.add_argument("--approval-consumed-marker", type=Path)
    parser.add_argument(
        "--include-live-config-overrides",
        action="store_true",
        help=(
            "Include the live-submit and real-funds config override flags in the generated "
            "adapter command. Armed mode keeps these disabled by default and requires "
            "explicit opt-in."
        ),
    )
    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute the resolved cargo command. Without this flag the script only prints the invocation plan.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    invocation = build_invocation(
        package_dir=resolve(args.package_dir),
        env_file=resolve(args.env_file),
        secrets_env_file=resolve(args.secrets_env_file) if args.secrets_env_file else None,
        mode=args.mode,
        daily_used_notional_usd=args.daily_used_notional_usd,
        idempotency_key=args.idempotency_key,
        execution_id=args.execution_id,
        plan_hash=args.plan_hash,
        report_file=resolve(args.report_file) if args.report_file else None,
        approval_consumed_marker=resolve(args.approval_consumed_marker)
        if args.approval_consumed_marker
        else None,
        include_live_config_overrides=args.include_live_config_overrides,
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
