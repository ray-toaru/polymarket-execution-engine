#!/usr/bin/env python3
"""Prepare the full local review bundle up to dual-control packet generation."""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
RUNTIME_BUNDLE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_canary_runtime_bundle.py"
DUAL_CONTROL_TEMPLATE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_dual_control_review_template.py"
REVIEW_PACKET_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_dual_control_review_packet.py"
APPROVAL_REQUEST_SCRIPT = ROOT / "validation" / "prepare_operator_approval_request_helpers.py"


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def load_approval_request_module():
    return load_module(APPROVAL_REQUEST_SCRIPT, "prepare_operator_approval_request_helpers")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--source-env-file", required=True, type=Path)
    parser.add_argument("--runtime-env-output", required=True, type=Path)
    parser.add_argument("--approval-request-output", required=True, type=Path)
    parser.add_argument("--dual-control-template-output", required=True, type=Path)
    parser.add_argument("--review-packet-output-dir", required=True, type=Path)
    parser.add_argument("--candidate-market-file", required=True, type=Path)
    parser.add_argument("--runtime-truth-file", required=True, type=Path)
    parser.add_argument("--release-zip", type=Path)
    parser.add_argument("--root-ci-run-id", required=True)
    parser.add_argument("--hermes-ci-run-id", required=True)
    parser.add_argument("--execution-engine-ci-run-id", required=True)
    parser.add_argument("--credentialed-sdk-run-id", required=True)
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


def resolve(path: Path) -> Path:
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def prepare_review_bundle(
    *,
    profile: str,
    source_env_file: Path,
    runtime_env_output: Path,
    approval_request_output: Path,
    dual_control_template_output: Path,
    review_packet_output_dir: Path,
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
    runtime_bundle = load_module(RUNTIME_BUNDLE_SCRIPT, "prepare_canary_runtime_bundle")
    dual_control_template = load_module(
        DUAL_CONTROL_TEMPLATE_SCRIPT, "prepare_dual_control_review_template"
    )
    review_packet = load_module(REVIEW_PACKET_SCRIPT, "prepare_dual_control_review_packet")

    bundle = runtime_bundle.prepare_bundle(
        profile=profile,
        source_env_file=source_env_file,
        runtime_env_output=runtime_env_output,
        approval_request_output=approval_request_output,
        candidate_market_file=candidate_market_file,
        runtime_truth_file=runtime_truth_file,
        release_zip=release_zip,
        root_ci_run_id=root_ci_run_id,
        hermes_ci_run_id=hermes_ci_run_id,
        execution_engine_ci_run_id=execution_engine_ci_run_id,
        credentialed_sdk_run_id=credentialed_sdk_run_id,
        operator_identity_ref=operator_identity_ref,
        approval_ticket_ref=approval_ticket_ref,
        max_order_notional_usd=max_order_notional_usd,
        max_daily_notional_usd=max_daily_notional_usd,
        valid_for_minutes=valid_for_minutes,
        write_runtime_secrets=write_runtime_secrets,
    )

    approval_request = dual_control_template.load_json(approval_request_output)
    template = dual_control_template.build_template(
        approval_request,
        approval_request_sha256=dual_control_template.sha256(approval_request_output),
    )
    dual_control_template_output.parent.mkdir(parents=True, exist_ok=True)
    dual_control_template_output.write_text(
        json.dumps(template, indent=2, sort_keys=True) + "\n"
    )

    release_zip_path = release_zip or load_approval_request_module().DEFAULT_RELEASE_ZIP
    release_sha = release_zip_path.with_suffix(release_zip_path.suffix + ".sha256")
    release_evidence = release_zip_path.with_suffix(release_zip_path.suffix + ".evidence.json")
    packet = review_packet.build_packet(
        output_dir=review_packet_output_dir,
        release_zip=release_zip_path,
        release_sha=release_sha,
        release_evidence=release_evidence,
        candidate=candidate_market_file,
        runtime_truth=runtime_truth_file,
        approval_request=approval_request_output,
        dual_control_template=dual_control_template_output,
    )
    (review_packet_output_dir / "packet.json").write_text(
        json.dumps(packet, indent=2, sort_keys=True) + "\n"
    )
    (review_packet_output_dir / "README.md").write_text(review_packet.packet_readme(packet))
    return {
        **bundle,
        "dual_control_template_output": str(dual_control_template_output),
        "review_packet_output_dir": str(review_packet_output_dir),
        "review_packet_status": packet["status"],
    }


def main() -> int:
    args = parse_args()
    release_zip = resolve(args.release_zip) if args.release_zip is not None else None
    result = prepare_review_bundle(
        profile=args.profile,
        source_env_file=resolve(args.source_env_file),
        runtime_env_output=resolve(args.runtime_env_output),
        approval_request_output=resolve(args.approval_request_output),
        dual_control_template_output=resolve(args.dual_control_template_output),
        review_packet_output_dir=resolve(args.review_packet_output_dir),
        candidate_market_file=resolve(args.candidate_market_file),
        runtime_truth_file=resolve(args.runtime_truth_file),
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
