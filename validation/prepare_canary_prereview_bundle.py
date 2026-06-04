#!/usr/bin/env python3
"""Prepare local canary materials from candidate discovery through review packet."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
PREPARE_CANDIDATE_SCRIPT = ROOT / "validation" / "prepare_canary_candidate_market.py"
REVIEW_BUNDLE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_canary_review_bundle.py"
APPROVAL_REQUEST_SCRIPT = ROOT / "validation" / "prepare_operator_approval_request_helpers.py"
ACTIVATE_PROFILE_SCRIPT = INTEGRATION_ROOT / "scripts" / "activate_pmx_profile.py"
STORE_TRUTH_SCRIPT = ROOT / "validation" / "run_real_funds_canary_store_truth_cli_preflight.py"


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
    parser.add_argument("--candidate-market-output", required=True, type=Path)
    parser.add_argument("--runtime-truth-output", required=True, type=Path)
    parser.add_argument("--candidate-audit-output", type=Path)
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
    return parser.parse_args()


def resolve(path: Path | None) -> Path | None:
    if path is None:
        return None
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def prepare_candidate(
    *,
    candidate_market_output: Path,
    candidate_audit_output: Path | None,
    human_review_ref: str,
    exchange_rule_evidence_ref: str,
    market_url: str | None,
    market_slug: str | None,
    outcome: str | None,
    gamma_url: str | None,
    clob_url: str | None,
    max_markets: int,
    max_clob_requests: int | None,
    target_size: str | None,
    max_order_notional_usd: str,
    max_spread_bps: int,
    exchange_rule_valid_for_minutes: int,
    timeout_seconds: float,
) -> dict[str, str]:
    candidate_module = load_module(PREPARE_CANDIDATE_SCRIPT, "prepare_canary_candidate_market")
    namespace = argparse.Namespace(
        output=candidate_market_output,
        audit_output=candidate_audit_output,
        human_review_ref=human_review_ref,
        exchange_rule_evidence_ref=exchange_rule_evidence_ref,
        gamma_url=gamma_url or candidate_module.DEFAULT_GAMMA_URL,
        clob_url=clob_url or candidate_module.DEFAULT_CLOB_URL,
        market_url=market_url,
        market_slug=market_slug,
        outcome=outcome,
        max_markets=max_markets,
        max_clob_requests=max_clob_requests,
        target_size=target_size,
        max_order_notional_usd=max_order_notional_usd,
        max_spread_bps=max_spread_bps,
        exchange_rule_valid_for_minutes=exchange_rule_valid_for_minutes,
        timeout_seconds=timeout_seconds,
    )
    candidate, audit = candidate_module.scan(namespace)
    candidate_module.write_json(candidate_market_output, candidate.to_engine_json())
    if candidate_audit_output is not None:
        candidate_module.write_json(candidate_audit_output, audit)
    return {
        "candidate_market_output": str(candidate_market_output),
        "candidate_audit_output": str(candidate_audit_output) if candidate_audit_output else "",
    }


def prepare_runtime_truth(
    *,
    runtime_truth_output: Path,
    candidate_market_file: Path,
    release_zip: Path | None,
    account_id: str | None = None,
) -> dict[str, str]:
    approval_module = load_approval_request_module()
    store_truth = load_module(STORE_TRUTH_SCRIPT, "run_real_funds_canary_store_truth_cli_preflight")
    release_zip_path = release_zip or approval_module.DEFAULT_RELEASE_ZIP
    sidecar = approval_module.load_release_sidecar(release_zip_path)
    artifact_sha256 = sidecar["artifact_sha256"]
    workspace_manifest_sha256 = sidecar["workspace_manifest_sha256"]
    archived_manifest_sha256 = sidecar["archived_manifest_sha256"]

    previous_candidate = os.environ.get("PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE")
    os.environ["PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE"] = str(candidate_market_file)
    try:
        url = store_truth.database_url()
        store_truth.check_database_connectivity(url)
        store_truth.build_cli()
        suffix = str(time.time_ns())
        account_id = account_id or f"acct-store-truth-{suffix}"
        condition_id = f"cond-store-truth-{suffix}"
        store_truth.seed_runtime_truth(url, account_id, condition_id)
        with tempfile.TemporaryDirectory(prefix="pmx-store-truth-cli-") as tmp_dir:
            report = store_truth.run_cli(
                Path(tmp_dir),
                account_id,
                condition_id,
                artifact_sha256=artifact_sha256,
                workspace_manifest_sha256=workspace_manifest_sha256,
                archived_manifest_sha256=archived_manifest_sha256,
            )
        failures: list[str] = []
        if report.get("status") != "preflight_ready":
            failures.append("CLI did not report preflight_ready")
        for key, expected in [
            ("posted", False),
            ("remote_side_effects", False),
            ("raw_signed_order_exposed", False),
            ("live_submit_allowed", True),
            ("real_funds_canary_allowed", True),
        ]:
            if report.get(key) is not expected:
                failures.append(f"unexpected {key}: {report.get(key)!r}")
        if failures:
            raise SystemExit(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        runtime_truth = store_truth.runtime_truth_document(
            account_id,
            condition_id,
            report,
            artifact_sha256=artifact_sha256,
            workspace_manifest_sha256=workspace_manifest_sha256,
            archived_manifest_sha256=archived_manifest_sha256,
        )
        runtime_truth_output.parent.mkdir(parents=True, exist_ok=True)
        store_truth.write_json(runtime_truth_output, runtime_truth)
        return {
            "runtime_truth_output": str(runtime_truth_output),
            "runtime_truth_status": runtime_truth["preflight_report"]["status"],
        }
    finally:
        if previous_candidate is None:
            os.environ.pop("PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE", None)
        else:
            os.environ["PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE"] = previous_candidate


def activate_runtime_profile_env(
    *,
    profile: str,
    source_env_file: Path,
    runtime_env_output: Path,
    write_runtime_secrets: bool = False,
) -> dict[str, str]:
    activate = load_module(ACTIVATE_PROFILE_SCRIPT, "activate_pmx_profile")
    source_values = activate.load_profile_source(source_env_file)
    activated = activate.activate_profile(profile, source_values)
    activate.write_runtime_env(
        runtime_env_output,
        activated,
        write_secrets=write_runtime_secrets,
    )
    os.environ.update(activated)
    return {
        "profile": activated["PMX_ACTIVE_ACCOUNT_PROFILE"],
        "account_id": activated["PMX_ACTIVE_ACCOUNT_ID"],
        "active_profile_ref": activated["PMX_ACTIVE_PROFILE_REF"],
        "runtime_env_output": str(runtime_env_output),
        "secrets_included": write_runtime_secrets,
    }


def prepare_prereview_bundle(
    *,
    profile: str,
    source_env_file: Path,
    runtime_env_output: Path,
    approval_request_output: Path,
    dual_control_template_output: Path,
    review_packet_output_dir: Path,
    candidate_market_output: Path,
    runtime_truth_output: Path,
    candidate_audit_output: Path | None,
    release_zip: Path | None,
    root_ci_run_id: str,
    hermes_ci_run_id: str,
    execution_engine_ci_run_id: str,
    credentialed_sdk_run_id: str,
    operator_identity_ref: str,
    approval_ticket_ref: str,
    human_review_ref: str,
    exchange_rule_evidence_ref: str,
    market_url: str | None,
    market_slug: str | None,
    outcome: str | None,
    gamma_url: str | None,
    clob_url: str | None,
    max_markets: int,
    max_clob_requests: int | None,
    target_size: str | None,
    max_order_notional_usd: str,
    max_daily_notional_usd: str,
    max_spread_bps: int,
    exchange_rule_valid_for_minutes: int,
    timeout_seconds: float,
    valid_for_minutes: int,
    write_runtime_secrets: bool = False,
) -> dict[str, str]:
    candidate_result = prepare_candidate(
        candidate_market_output=candidate_market_output,
        candidate_audit_output=candidate_audit_output,
        human_review_ref=human_review_ref,
        exchange_rule_evidence_ref=exchange_rule_evidence_ref,
        market_url=market_url,
        market_slug=market_slug,
        outcome=outcome,
        gamma_url=gamma_url,
        clob_url=clob_url,
        max_markets=max_markets,
        max_clob_requests=max_clob_requests,
        target_size=target_size,
        max_order_notional_usd=max_order_notional_usd,
        max_spread_bps=max_spread_bps,
        exchange_rule_valid_for_minutes=exchange_rule_valid_for_minutes,
        timeout_seconds=timeout_seconds,
    )
    runtime_profile_result = activate_runtime_profile_env(
        profile=profile,
        source_env_file=source_env_file,
        runtime_env_output=runtime_env_output,
        write_runtime_secrets=write_runtime_secrets,
    )
    runtime_truth_result = prepare_runtime_truth(
        runtime_truth_output=runtime_truth_output,
        candidate_market_file=candidate_market_output,
        release_zip=release_zip,
        account_id=runtime_profile_result["account_id"],
    )
    review_bundle = load_module(REVIEW_BUNDLE_SCRIPT, "prepare_canary_review_bundle")
    bundle = review_bundle.prepare_review_bundle(
        profile=profile,
        source_env_file=source_env_file,
        runtime_env_output=runtime_env_output,
        approval_request_output=approval_request_output,
        dual_control_template_output=dual_control_template_output,
        review_packet_output_dir=review_packet_output_dir,
        candidate_market_file=candidate_market_output,
        runtime_truth_file=runtime_truth_output,
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
    return {**candidate_result, **runtime_profile_result, **runtime_truth_result, **bundle}


def main() -> int:
    args = parse_args()
    result = prepare_prereview_bundle(
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
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
