#!/usr/bin/env python3
"""Prepare a fresh canary review packet and optionally promote it to reviewed-go."""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
PREREVIEW_BUNDLE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_canary_prereview_bundle.py"
REVIEWED_GO_BUNDLE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_canary_reviewed_go_bundle.py"


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def resolve(path: Path | None) -> Path | None:
    if path is None:
        return None
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--source-env-file", required=True, type=Path)
    parser.add_argument("--runtime-env-output", required=True, type=Path)
    parser.add_argument("--candidate-market-output", required=True, type=Path)
    parser.add_argument("--candidate-audit-output", type=Path)
    parser.add_argument("--runtime-truth-output", required=True, type=Path)
    parser.add_argument("--approval-request-output", required=True, type=Path)
    parser.add_argument("--dual-control-template-output", required=True, type=Path)
    parser.add_argument("--review-packet-output-dir", required=True, type=Path)
    parser.add_argument("--release-zip", type=Path)
    parser.add_argument("--root-ci-run-id", required=True)
    parser.add_argument("--hermes-ci-run-id", required=True)
    parser.add_argument("--execution-engine-ci-run-id", required=True)
    parser.add_argument("--credentialed-sdk-run-id", default="local-current-gates-20260523")
    parser.add_argument("--operator-identity-ref", required=True)
    parser.add_argument("--approval-ticket-ref", required=True)
    parser.add_argument("--human-review-ref", required=True)
    parser.add_argument("--exchange-rule-evidence-ref", required=True)
    parser.add_argument("--market-url")
    parser.add_argument("--market-slug")
    parser.add_argument("--outcome")
    parser.add_argument("--gamma-url")
    parser.add_argument("--clob-url")
    parser.add_argument("--max-markets", type=int, default=200)
    parser.add_argument("--max-clob-requests", type=int)
    parser.add_argument("--target-size")
    parser.add_argument("--max-order-notional-usd", default="0.20")
    parser.add_argument("--max-daily-notional-usd", default="0.20")
    parser.add_argument("--max-spread-bps", type=int, default=100)
    parser.add_argument("--exchange-rule-valid-for-minutes", type=int, default=5)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    parser.add_argument("--valid-for-minutes", type=int, default=15)
    parser.add_argument(
        "--write-runtime-secrets",
        action="store_true",
        help="Also write the companion .env.runtime.secrets file. Without this flag only runtime identity is emitted.",
    )
    parser.add_argument("--approved-dual-control-review-file", type=Path)
    parser.add_argument("--external-references-file", type=Path)
    parser.add_argument("--reviewed-go-output-dir", type=Path)
    parser.add_argument("--decision-id")
    parser.add_argument("--decision-reason", default="approved by independent reviewer")
    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute the workflow. Without this flag the script only prints the workflow plan.",
    )
    return parser.parse_args()


def build_workflow_plan(args: argparse.Namespace) -> dict[str, Any]:
    prereview = {
        "name": "prereview_bundle",
        "script": str(PREREVIEW_BUNDLE_SCRIPT.relative_to(INTEGRATION_ROOT)),
        "outputs": {
            "candidate_market_output": str(resolve(args.candidate_market_output)),
            "candidate_audit_output": str(resolve(args.candidate_audit_output))
            if args.candidate_audit_output is not None
            else None,
            "runtime_truth_output": str(resolve(args.runtime_truth_output)),
            "runtime_env_output": str(resolve(args.runtime_env_output)),
            "write_runtime_secrets": args.write_runtime_secrets,
            "approval_request_output": str(resolve(args.approval_request_output)),
            "dual_control_template_output": str(resolve(args.dual_control_template_output)),
            "review_packet_output_dir": str(resolve(args.review_packet_output_dir)),
        },
    }
    promotion_enabled = all(
        value is not None
        for value in (
            args.approved_dual_control_review_file,
            args.external_references_file,
            args.reviewed_go_output_dir,
        )
    )
    promotion = {
        "name": "reviewed_go_promotion",
        "script": str(REVIEWED_GO_BUNDLE_SCRIPT.relative_to(INTEGRATION_ROOT)),
        "enabled": promotion_enabled,
        "blocked_reason": None
        if promotion_enabled
        else "approved dual-control review, external references, and reviewed-go output dir are required",
        "inputs": {
            "approved_dual_control_review_file": str(resolve(args.approved_dual_control_review_file))
            if args.approved_dual_control_review_file is not None
            else None,
            "external_references_file": str(resolve(args.external_references_file))
            if args.external_references_file is not None
            else None,
        },
        "outputs": {
            "reviewed_go_output_dir": str(resolve(args.reviewed_go_output_dir))
            if args.reviewed_go_output_dir is not None
            else None
        },
    }
    return {
        "status": "ready",
        "workflow": "reviewed_go_decision_chain",
        "execution_plane": "polymarket-execution-engine/validation/run_reviewed_go_decision_workflow.py",
        "stages": [prereview, promotion],
    }


def execute_workflow(args: argparse.Namespace) -> dict[str, Any]:
    prereview_module = load_module(PREREVIEW_BUNDLE_SCRIPT, "prepare_canary_prereview_bundle")
    prereview_result = prereview_module.prepare_prereview_bundle(
        profile=args.profile,
        source_env_file=resolve(args.source_env_file),
        runtime_env_output=resolve(args.runtime_env_output),
        approval_request_output=resolve(args.approval_request_output),
        dual_control_template_output=resolve(args.dual_control_template_output),
        review_packet_output_dir=resolve(args.review_packet_output_dir),
        candidate_market_output=resolve(args.candidate_market_output),
        runtime_truth_output=resolve(args.runtime_truth_output),
        candidate_audit_output=resolve(args.candidate_audit_output),
        release_zip=resolve(args.release_zip),
        root_ci_run_id=args.root_ci_run_id,
        hermes_ci_run_id=args.hermes_ci_run_id,
        execution_engine_ci_run_id=args.execution_engine_ci_run_id,
        credentialed_sdk_run_id=args.credentialed_sdk_run_id,
        operator_identity_ref=args.operator_identity_ref,
        approval_ticket_ref=args.approval_ticket_ref,
        human_review_ref=args.human_review_ref,
        exchange_rule_evidence_ref=args.exchange_rule_evidence_ref,
        market_url=args.market_url,
        market_slug=args.market_slug,
        outcome=args.outcome,
        gamma_url=args.gamma_url,
        clob_url=args.clob_url,
        max_markets=args.max_markets,
        max_clob_requests=args.max_clob_requests,
        target_size=args.target_size,
        max_order_notional_usd=args.max_order_notional_usd,
        max_daily_notional_usd=args.max_daily_notional_usd,
        max_spread_bps=args.max_spread_bps,
        exchange_rule_valid_for_minutes=args.exchange_rule_valid_for_minutes,
        timeout_seconds=args.timeout_seconds,
        valid_for_minutes=args.valid_for_minutes,
        write_runtime_secrets=args.write_runtime_secrets,
    )
    result: dict[str, Any] = {
        "status": "review_packet_ready_requires_independent_review",
        "workflow": "reviewed_go_decision_chain",
        "execution_plane": "polymarket-execution-engine/validation/run_reviewed_go_decision_workflow.py",
        "prereview": prereview_result,
        "promotion": {
            "status": "blocked",
            "blocked_reason": "approved dual-control review, external references, and reviewed-go output dir are required",
        },
    }
    promotion_inputs_present = all(
        value is not None
        for value in (
            args.approved_dual_control_review_file,
            args.external_references_file,
            args.reviewed_go_output_dir,
        )
    )
    if not promotion_inputs_present:
        return result

    reviewed_go_module = load_module(
        REVIEWED_GO_BUNDLE_SCRIPT, "prepare_canary_reviewed_go_bundle"
    )
    promotion_result = reviewed_go_module.prepare_reviewed_go_bundle(
        review_packet_dir=resolve(args.review_packet_output_dir),
        approved_dual_control_review_file=resolve(args.approved_dual_control_review_file),
        external_references_file=resolve(args.external_references_file),
        output_dir=resolve(args.reviewed_go_output_dir),
        decision_id=args.decision_id,
        decision_reason=args.decision_reason,
    )
    result["status"] = "reviewed_go_package_ready"
    result["promotion"] = promotion_result
    return result


def main() -> int:
    args = parse_args()
    plan = build_workflow_plan(args)
    if not args.run:
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0
    result = execute_workflow(args)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
