#!/usr/bin/env python3
"""Activate one local canary profile and build the matching approval request."""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
ACTIVATE_SCRIPT = INTEGRATION_ROOT / "scripts" / "activate_pmx_profile.py"
APPROVAL_SCRIPT = ROOT / "validation" / "prepare_operator_approval_request_helpers.py"


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def load_approval_request_module():
    return load_module(APPROVAL_SCRIPT, "prepare_operator_approval_request_helpers")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--source-env-file", required=True, type=Path)
    parser.add_argument("--runtime-env-output", required=True, type=Path)
    parser.add_argument("--approval-request-output", required=True, type=Path)
    parser.add_argument("--candidate-market-file", required=True, type=Path)
    parser.add_argument("--runtime-truth-file", required=True, type=Path)
    parser.add_argument("--release-zip", type=Path)
    parser.add_argument("--root-ci-run-id", required=True)
    parser.add_argument("--hermes-ci-run-id", required=True)
    parser.add_argument("--execution-engine-ci-run-id", required=True)
    parser.add_argument("--credentialed-sdk-run-id", default="local-current-gates-20260523")
    parser.add_argument("--operator-identity-ref", required=True)
    parser.add_argument("--approval-ticket-ref", required=True)
    parser.add_argument("--max-order-notional-usd", default="0.20")
    parser.add_argument("--max-daily-notional-usd", default="0.20")
    parser.add_argument("--valid-for-minutes", type=int, default=15)
    parser.add_argument(
        "--write-runtime-secrets",
        action="store_true",
        help="Also write the companion .env.runtime.secrets file. Without this flag only runtime identity is emitted.",
    )
    return parser.parse_args()


def prepare_bundle(
    *,
    profile: str,
    source_env_file: Path,
    runtime_env_output: Path,
    approval_request_output: Path,
    candidate_market_file: Path,
    runtime_truth_file: Path,
    release_zip: Path | None,
    root_ci_run_id: str,
    hermes_ci_run_id: str,
    execution_engine_ci_run_id: str,
    credentialed_sdk_run_id: str,
    operator_identity_ref: str,
    approval_ticket_ref: str,
    max_order_notional_usd: str,
    max_daily_notional_usd: str,
    valid_for_minutes: int,
    write_runtime_secrets: bool = False,
) -> dict[str, str]:
    activate = load_module(ACTIVATE_SCRIPT, "activate_pmx_profile")
    approval = load_approval_request_module()
    source_values = activate.load_profile_source(source_env_file)
    activated = activate.activate_profile(profile, source_values)
    activate.write_runtime_env(
        runtime_env_output,
        activated,
        write_secrets=write_runtime_secrets,
    )

    account_id, active_profile_ref = approval.resolve_runtime_identity(
        runtime_env_file=runtime_env_output,
        account_id=None,
        active_profile_ref=None,
    )
    release_zip_path = release_zip or approval.DEFAULT_RELEASE_ZIP
    sidecar = approval.load_release_sidecar(release_zip_path)
    candidate = approval.load_json(candidate_market_file)
    runtime_truth = approval.load_json(runtime_truth_file)
    runtime_summary = approval.validate_runtime_truth(runtime_truth, expected_account_id=account_id)
    max_order_notional = approval.decimal_value(max_order_notional_usd, "max_order_notional_usd")
    max_daily_notional = approval.decimal_value(max_daily_notional_usd, "max_daily_notional_usd")
    candidate_limits = approval.validate_candidate(candidate, max_order_notional)
    runtime_artifact = approval.require_sha256(
        runtime_truth.get("artifact_sha256"),
        "runtime truth artifact_sha256",
    )
    if runtime_artifact != sidecar["artifact_sha256"]:
        raise SystemExit("runtime truth artifact hash does not match release sidecar")
    request = approval.build_request(
        account_id=account_id,
        condition_id=runtime_summary["condition_id"],
        active_profile_ref=active_profile_ref,
        operator_identity_ref=operator_identity_ref,
        approval_ticket_ref=approval_ticket_ref,
        candidate_market_file=candidate_market_file,
        runtime_truth_file=runtime_truth_file,
        runtime_gate_snapshot=runtime_summary["gate_snapshot"],
        runtime_gate_evidence_refs=runtime_summary["gate_evidence_refs"],
        sidecar=sidecar,
        candidate_limits=candidate_limits,
        max_order_notional=max_order_notional,
        max_daily_notional=max_daily_notional,
        root_ci_run_id=root_ci_run_id,
        hermes_ci_run_id=hermes_ci_run_id,
        execution_engine_ci_run_id=execution_engine_ci_run_id,
        credentialed_sdk_run_id=credentialed_sdk_run_id,
        valid_for_minutes=valid_for_minutes,
    )
    approval_request_output.parent.mkdir(parents=True, exist_ok=True)
    approval_request_output.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")
    return {
        "status": "pass",
        "profile": activated["PMX_ACTIVE_ACCOUNT_PROFILE"],
        "account_id": account_id,
        "active_profile_ref": active_profile_ref,
        "runtime_env_output": str(runtime_env_output),
        "approval_request_output": str(approval_request_output),
        "approval_hash": request["approval_hash"],
        "secrets_included": write_runtime_secrets,
    }


def main() -> int:
    args = parse_args()
    source_env = (
        args.source_env_file
        if args.source_env_file.is_absolute()
        else INTEGRATION_ROOT / args.source_env_file
    )
    runtime_env_output = (
        args.runtime_env_output
        if args.runtime_env_output.is_absolute()
        else INTEGRATION_ROOT / args.runtime_env_output
    )
    approval_request_output = (
        args.approval_request_output
        if args.approval_request_output.is_absolute()
        else INTEGRATION_ROOT / args.approval_request_output
    )
    candidate_market_file = (
        args.candidate_market_file
        if args.candidate_market_file.is_absolute()
        else INTEGRATION_ROOT / args.candidate_market_file
    )
    runtime_truth_file = (
        args.runtime_truth_file
        if args.runtime_truth_file.is_absolute()
        else INTEGRATION_ROOT / args.runtime_truth_file
    )
    release_zip = None
    if args.release_zip is not None:
        release_zip = (
            args.release_zip if args.release_zip.is_absolute() else INTEGRATION_ROOT / args.release_zip
        )
    result = prepare_bundle(
        profile=args.profile,
        source_env_file=source_env,
        runtime_env_output=runtime_env_output,
        approval_request_output=approval_request_output,
        candidate_market_file=candidate_market_file,
        runtime_truth_file=runtime_truth_file,
        release_zip=release_zip,
        root_ci_run_id=args.root_ci_run_id,
        hermes_ci_run_id=args.hermes_ci_run_id,
        execution_engine_ci_run_id=args.execution_engine_ci_run_id,
        credentialed_sdk_run_id=args.credentialed_sdk_run_id,
        operator_identity_ref=args.operator_identity_ref,
        approval_ticket_ref=args.approval_ticket_ref,
        max_order_notional_usd=args.max_order_notional_usd,
        max_daily_notional_usd=args.max_daily_notional_usd,
        valid_for_minutes=args.valid_for_minutes,
        write_runtime_secrets=args.write_runtime_secrets,
    )
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
