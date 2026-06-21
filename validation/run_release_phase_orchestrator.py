#!/usr/bin/env python3
"""Plan or run the current root release-phase workflows without weakening gates."""
from __future__ import annotations

import argparse
import importlib.util
import json
import hashlib
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
PRODUCTION_CONTROL_SUITE = INTEGRATION_ROOT / "scripts" / "run_production_control_suite.py"
DEPLOYMENT_VALIDATION_SUITE = INTEGRATION_ROOT / "scripts" / "run_deployment_validation_suite.py"
LIVE_SUBMIT_PROMOTION_SUITE = INTEGRATION_ROOT / "scripts" / "run_live_submit_promotion_suite.py"
REVIEWED_GO_DECISION_WORKFLOW = INTEGRATION_ROOT / "scripts" / "run_reviewed_go_decision_workflow.py"
CONTRACT_VALIDATION_SCRIPT = INTEGRATION_ROOT / "scripts" / "validate_contracts.py"


def non_authorizing_report_fields() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "authorization_scope": "no_authorization",
        "production_ready": False,
        "live_trading_ready": False,
    }


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


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def contract_validation_output_dir(args: argparse.Namespace) -> Path:
    if args.output_dir:
        return resolve(args.output_dir) / "contract-validation"
    return ROOT / "dist" / "release-phase-orchestrator" / "contract-validation"


def build_contract_validation_plan(args: argparse.Namespace) -> dict[str, Any]:
    report_file = contract_validation_output_dir(args) / "contract-validation.report.json"
    log_file = contract_validation_output_dir(args) / "contract-validation.stdout.json"
    return {
        "status": "ready",
        "suite": "contract_validation",
        "command": [
            sys.executable,
            str(CONTRACT_VALIDATION_SCRIPT),
            "--report-file",
            str(report_file),
        ],
        "report_file": str(report_file),
        "stdout_file": str(log_file),
    }


def execute_contract_validation(plan: dict[str, Any]) -> dict[str, Any]:
    report_file = Path(plan["report_file"])
    stdout_file = Path(plan["stdout_file"])
    report_file.parent.mkdir(parents=True, exist_ok=True)
    completed = subprocess.run(
        plan["command"],
        cwd=INTEGRATION_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    stdout_file.write_text(completed.stdout)
    if completed.stderr:
        stdout_file.with_suffix(stdout_file.suffix + ".stderr").write_text(completed.stderr)
    if report_file.exists():
        report = json.loads(report_file.read_text())
    else:
        report = {
            "status": "fail",
            "failed_check_count": None,
            "failed_check_ids": [],
            "checks": [],
        }
    result = {
        "status": "pass" if completed.returncode == 0 and report.get("status") == "ok" else "fail",
        "suite": "contract_validation",
        "command": plan["command"],
        "report_file": str(report_file),
        "stdout_file": str(stdout_file),
        "returncode": completed.returncode,
        "report_status": report.get("status"),
        "failed_check_count": report.get("failed_check_count"),
        "failed_check_ids": report.get("failed_check_ids", []),
    }
    if report_file.exists():
        result["report_sha256"] = sha256(report_file)
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-zip", type=Path)
    parser.add_argument("--output-dir", type=Path)

    parser.add_argument("--profile")
    parser.add_argument("--source-env-file", type=Path)
    parser.add_argument("--runtime-env-output", type=Path)
    parser.add_argument(
        "--write-runtime-secrets",
        action="store_true",
        help="Also write the companion .env.runtime.secrets file. Without this flag only runtime identity is emitted.",
    )
    parser.add_argument("--candidate-market-output", type=Path)
    parser.add_argument("--candidate-audit-output", type=Path)
    parser.add_argument("--runtime-truth-output", type=Path)
    parser.add_argument("--approval-request-output", type=Path)
    parser.add_argument("--dual-control-template-output", type=Path)
    parser.add_argument("--review-packet-output-dir", type=Path)
    parser.add_argument("--root-ci-run-id")
    parser.add_argument("--hermes-ci-run-id")
    parser.add_argument("--execution-engine-ci-run-id")
    parser.add_argument("--credentialed-sdk-run-id", required=True)
    parser.add_argument("--operator-identity-ref")
    parser.add_argument("--approval-ticket-ref")
    parser.add_argument("--human-review-ref")
    parser.add_argument("--exchange-rule-evidence-ref")
    parser.add_argument("--market-url")
    parser.add_argument("--market-slug")
    parser.add_argument("--outcome")
    parser.add_argument("--gamma-url")
    parser.add_argument("--clob-url")
    parser.add_argument("--max-markets", type=int, default=200)
    parser.add_argument("--target-size")
    parser.add_argument("--max-order-notional-usd", default="0.20")
    parser.add_argument("--max-daily-notional-usd", default="0.20")
    parser.add_argument("--max-spread-bps", type=int, default=100)
    parser.add_argument("--exchange-rule-valid-for-minutes", type=int, default=5)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    parser.add_argument("--valid-for-minutes", type=int, default=15)
    parser.add_argument("--approved-dual-control-review-file", type=Path)
    parser.add_argument("--canonical-dual-control-review-file", type=Path)
    parser.add_argument("--review-signature-file", type=Path)
    parser.add_argument("--reviewer-registry-file", type=Path)
    parser.add_argument("--external-references-file", type=Path)
    parser.add_argument("--reviewed-go-output-dir", type=Path)
    parser.add_argument("--decision-id")
    parser.add_argument("--decision-reason", default="approved by independent reviewer")

    parser.add_argument(
        "--run",
        action="store_true",
        help="Execute the orchestrator. Without this flag the script only prints the stage plans.",
    )
    return parser.parse_args()


def build_stage_plans(args: argparse.Namespace) -> dict[str, Any]:
    output_dir = resolve(args.output_dir) if args.output_dir else None

    production_module = load_module(PRODUCTION_CONTROL_SUITE, "run_production_control_suite")
    deployment_module = load_module(DEPLOYMENT_VALIDATION_SUITE, "run_deployment_validation_suite")
    promotion_module = load_module(LIVE_SUBMIT_PROMOTION_SUITE, "run_live_submit_promotion_suite")

    plans: dict[str, Any] = {
        **non_authorizing_report_fields(),
        "status": "ready",
        "workflow": "release_phase_orchestrator",
        "stages": {
            "contract_validation": build_contract_validation_plan(args),
            "production_control": production_module.build_suite_plan(
                release_zip=args.release_zip,
                output_dir=(output_dir / "production-control") if output_dir else None,
            ),
            "deployment_validation": deployment_module.build_suite_plan(
                release_zip=args.release_zip,
                output_dir=(output_dir / "deployment-validation") if output_dir else None,
            ),
            "live_submit_promotion": promotion_module.build_suite_plan(
                output_dir=(output_dir / "live-submit-promotion") if output_dir else None,
            ),
        },
    }

    reviewed_go_inputs_present = all(
        value is not None
        for value in (
            args.profile,
            args.source_env_file,
            args.runtime_env_output,
            args.candidate_market_output,
            args.runtime_truth_output,
            args.approval_request_output,
            args.dual_control_template_output,
            args.review_packet_output_dir,
            args.root_ci_run_id,
            args.hermes_ci_run_id,
            args.execution_engine_ci_run_id,
            args.operator_identity_ref,
            args.approval_ticket_ref,
            args.human_review_ref,
        )
    )
    if reviewed_go_inputs_present:
        reviewed_go_module = load_module(
            REVIEWED_GO_DECISION_WORKFLOW, "run_reviewed_go_decision_workflow"
        )
        reviewed_go_args = reviewed_go_module.argparse.Namespace(
            profile=args.profile,
            source_env_file=resolve(args.source_env_file),
            runtime_env_output=resolve(args.runtime_env_output),
            write_runtime_secrets=args.write_runtime_secrets,
            candidate_market_output=resolve(args.candidate_market_output),
            candidate_audit_output=resolve(args.candidate_audit_output),
            runtime_truth_output=resolve(args.runtime_truth_output),
            approval_request_output=resolve(args.approval_request_output),
            dual_control_template_output=resolve(args.dual_control_template_output),
            review_packet_output_dir=resolve(args.review_packet_output_dir),
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
            target_size=args.target_size,
            max_order_notional_usd=args.max_order_notional_usd,
            max_daily_notional_usd=args.max_daily_notional_usd,
            max_spread_bps=args.max_spread_bps,
            exchange_rule_valid_for_minutes=args.exchange_rule_valid_for_minutes,
            timeout_seconds=args.timeout_seconds,
            valid_for_minutes=args.valid_for_minutes,
            approved_dual_control_review_file=resolve(args.approved_dual_control_review_file),
            canonical_dual_control_review_file=resolve(args.canonical_dual_control_review_file),
            review_signature_file=resolve(args.review_signature_file),
            reviewer_registry_file=resolve(args.reviewer_registry_file),
            external_references_file=resolve(args.external_references_file),
            reviewed_go_output_dir=resolve(args.reviewed_go_output_dir),
            decision_id=args.decision_id,
            decision_reason=args.decision_reason,
            run=False,
        )
        plans["stages"]["reviewed_go_decision_chain"] = reviewed_go_module.build_workflow_plan(
            reviewed_go_args
        )
    else:
        plans["stages"]["reviewed_go_decision_chain"] = {
            "status": "blocked",
            "workflow": "reviewed_go_decision_chain",
            "blocked_reason": "reviewed-go prereview inputs are incomplete",
        }
    return plans


def execute_orchestrator(args: argparse.Namespace) -> dict[str, Any]:
    plans = build_stage_plans(args)

    production_module = load_module(PRODUCTION_CONTROL_SUITE, "run_production_control_suite")
    deployment_module = load_module(DEPLOYMENT_VALIDATION_SUITE, "run_deployment_validation_suite")
    promotion_module = load_module(LIVE_SUBMIT_PROMOTION_SUITE, "run_live_submit_promotion_suite")

    stage_results: dict[str, Any] = {}
    stage_results["contract_validation"] = execute_contract_validation(
        plans["stages"]["contract_validation"]
    )
    if stage_results["contract_validation"]["status"] == "fail":
        blocked = {
            "status": "blocked",
            "blocked_reason": "contract_validation_failed",
            "depends_on": "contract_validation",
        }
        stage_results["production_control"] = {
            **blocked,
            "suite": plans["stages"]["production_control"]["suite"],
        }
        stage_results["deployment_validation"] = {
            **blocked,
            "suite": plans["stages"]["deployment_validation"]["suite"],
        }
        reviewed_go_stage = plans["stages"]["reviewed_go_decision_chain"]
        reviewed_go_name = reviewed_go_stage.get("workflow", "reviewed_go_decision_chain")
        stage_results["live_submit_promotion"] = {
            **blocked,
            "suite": plans["stages"]["live_submit_promotion"]["suite"],
        }
        stage_results["reviewed_go_decision_chain"] = {
            **blocked,
            "workflow": reviewed_go_name,
        }
        return {
            **non_authorizing_report_fields(),
            "status": "fail",
            "workflow": "release_phase_orchestrator",
            "stages": stage_results,
        }
    stage_results["production_control"] = production_module.execute_suite(
        plans["stages"]["production_control"]
    )
    stage_results["deployment_validation"] = deployment_module.execute_suite(
        plans["stages"]["deployment_validation"]
    )
    stage_results["live_submit_promotion"] = promotion_module.execute_suite(
        plans["stages"]["live_submit_promotion"]
    )

    reviewed_go_stage = plans["stages"]["reviewed_go_decision_chain"]
    if reviewed_go_stage.get("status") == "blocked":
        stage_results["reviewed_go_decision_chain"] = reviewed_go_stage
    else:
        reviewed_go_module = load_module(
            REVIEWED_GO_DECISION_WORKFLOW, "run_reviewed_go_decision_workflow"
        )
        reviewed_go_args = parse_args_for_reviewed_go(args)
        stage_results["reviewed_go_decision_chain"] = reviewed_go_module.execute_workflow(
            reviewed_go_args
        )

    fail = any(
        result.get("status") in {"fail"}
        for result in stage_results.values()
        if isinstance(result, dict)
    )
    return {
        **non_authorizing_report_fields(),
        "status": "fail" if fail else "pass",
        "workflow": "release_phase_orchestrator",
        "stages": stage_results,
    }


def parse_args_for_reviewed_go(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        profile=args.profile,
        source_env_file=resolve(args.source_env_file),
        runtime_env_output=resolve(args.runtime_env_output),
        write_runtime_secrets=args.write_runtime_secrets,
        candidate_market_output=resolve(args.candidate_market_output),
        candidate_audit_output=resolve(args.candidate_audit_output),
        runtime_truth_output=resolve(args.runtime_truth_output),
        approval_request_output=resolve(args.approval_request_output),
        dual_control_template_output=resolve(args.dual_control_template_output),
        review_packet_output_dir=resolve(args.review_packet_output_dir),
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
        target_size=args.target_size,
        max_order_notional_usd=args.max_order_notional_usd,
        max_daily_notional_usd=args.max_daily_notional_usd,
        max_spread_bps=args.max_spread_bps,
        exchange_rule_valid_for_minutes=args.exchange_rule_valid_for_minutes,
        timeout_seconds=args.timeout_seconds,
        valid_for_minutes=args.valid_for_minutes,
        approved_dual_control_review_file=resolve(args.approved_dual_control_review_file),
        canonical_dual_control_review_file=resolve(args.canonical_dual_control_review_file),
        review_signature_file=resolve(args.review_signature_file),
        reviewer_registry_file=resolve(args.reviewer_registry_file),
        external_references_file=resolve(args.external_references_file),
        reviewed_go_output_dir=resolve(args.reviewed_go_output_dir),
        decision_id=args.decision_id,
        decision_reason=args.decision_reason,
        run=True,
    )


def main() -> int:
    args = parse_args()
    if not args.run:
        print(json.dumps(build_stage_plans(args), indent=2, sort_keys=True))
        return 0
    result = execute_orchestrator(args)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if result["status"] != "pass" else 0


if __name__ == "__main__":
    raise SystemExit(main())
