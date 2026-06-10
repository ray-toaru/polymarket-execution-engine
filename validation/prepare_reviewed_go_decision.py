#!/usr/bin/env python3
"""Promote a canary approval request into a reviewed-go decision."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
VERSION = (INTEGRATION_ROOT / "VERSION").read_text().strip()
VALIDATOR = ROOT / "validation" / "validate_controlled_canary_release_decision.py"


REQUIRED_EXTERNAL_REFS = {
    "secret_custody_ref": ("secret_custody", "provider_ref"),
    "operator_approval_ref": ("operator_approval", "ticket_ref"),
    "alert_routing_ref": ("alert_routing", "route_ref"),
    "dashboard_ref": ("alert_routing", "dashboard_ref"),
    "rollback_runbook_ref": ("runbooks", "rollback_runbook_ref"),
    "incident_runbook_ref": ("runbooks", "incident_runbook_ref"),
}
REVIEW_SIGNALS = [
    "artifact_hash_reviewed",
    "evidence_manifest_hash_reviewed",
    "market_candidate_reviewed",
    "operator_dual_control_reviewed",
    "secret_custody_reviewed",
    "alerting_reviewed",
    "rollback_reviewed",
    "runtime_health_reviewed",
    "reconcile_and_cancel_fallback_reviewed",
]
REQUIRED_DUAL_CONTROL_CHECKS = [
    "artifact_hash_reviewed",
    "evidence_manifest_hash_reviewed",
    "market_candidate_reviewed",
    "runtime_truth_reviewed",
    "risk_limits_reviewed",
    "secret_custody_reviewed",
    "alerting_reviewed",
    "rollback_reviewed",
    "reconcile_and_cancel_fallback_reviewed",
]
PREFLIGHT_GATE_FIELDS = [
    "preconditions_live_submit_would_pass",
    "preconditions_real_funds_canary_would_pass",
    "kill_switch_open",
    "runtime_worker_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_worker_healthy",
    "cancel_only_fallback_ready",
    "balance_allowance_checked",
]
DECISION_ID_RE = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def has_placeholder(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith("REPLACE_WITH_") or not value.strip()
    if isinstance(value, dict):
        return any(has_placeholder(child) for child in value.values())
    if isinstance(value, list):
        return any(has_placeholder(child) for child in value)
    return False


def require_sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(ch not in "0123456789abcdefABCDEF" for ch in value):
        raise SystemExit(f"{label} must be a 64-character SHA-256 hex digest")
    return value.lower()


def require_git_sha(value: object, label: str) -> str:
    if not isinstance(value, str) or len(value) not in {40, 64} or any(
        ch not in "0123456789abcdefABCDEF" for ch in value
    ):
        raise SystemExit(f"{label} must be a git SHA")
    return value.lower()


def require_nonempty_text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"{label} must be a non-empty string")
    return value.strip()


def require_concrete_text(value: object, label: str) -> str:
    text = require_nonempty_text(value, label)
    if text.startswith("REPLACE_WITH_"):
        raise SystemExit(f"{label} must not be a placeholder")
    return text


def require_decision_id(value: object) -> str:
    decision_id = require_concrete_text(value, "decision_id")
    if not DECISION_ID_RE.fullmatch(decision_id):
        raise SystemExit("decision_id must use [A-Za-z0-9._:-] and be <= 128 chars")
    return decision_id


def parse_time(value: object, label: str) -> datetime:
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"{label} must be an RFC3339 timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise SystemExit(f"{label} must be an RFC3339 timestamp") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def git_head(repo: Path) -> str:
    return subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()


def legacy_github_evidence_details(github_evidence: dict[str, Any]) -> dict[str, dict[str, str]]:
    """Normalize pre-detail approval requests for reviewed-go package migration."""
    timestamp = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    root_head = git_head(INTEGRATION_ROOT)
    hermes_head = git_head(INTEGRATION_ROOT / "hermes-polymarket-executor-adapter")
    engine_head = git_head(ROOT)
    workflow_run_ids = github_evidence.get("workflow_run_ids", {})
    if not isinstance(workflow_run_ids, dict):
        workflow_run_ids = {}
    return {
        "root_ci": {
            "run_id": str(workflow_run_ids.get("root_ci", "local-root-ci")),
            "workflow_name": "local root validation",
            "workflow_run_url": "local://root-ci",
            "commit_sha": root_head,
            "status": "local_passed",
            "timestamp": timestamp,
        },
        "hermes_ci": {
            "run_id": str(workflow_run_ids.get("hermes_ci", "local-hermes-ci")),
            "workflow_name": "local hermes validation",
            "workflow_run_url": "local://hermes-ci",
            "commit_sha": hermes_head,
            "status": "local_passed",
            "timestamp": timestamp,
        },
        "execution_engine_ci": {
            "run_id": str(workflow_run_ids.get("execution_engine_ci", "local-execution-engine-ci")),
            "workflow_name": "local execution engine validation",
            "workflow_run_url": "local://execution-engine-ci",
            "commit_sha": engine_head,
            "status": "local_passed",
            "timestamp": timestamp,
        },
        "credentialed_sdk": {
            "run_id": str(workflow_run_ids.get("credentialed_sdk", "not-applicable-non-live")),
            "workflow_name": "credentialed sdk gate",
            "workflow_run_url": "evidence://credentialed-sdk/not-applicable-non-live",
            "commit_sha": engine_head,
            "status": "not_applicable_non_live",
            "timestamp": timestamp,
        },
    }


def validate_approval_request(request: dict[str, Any]) -> None:
    if request.get("release_posture") != "non_live_hardened":
        raise SystemExit("approval request release_posture must be non_live_hardened")
    if request.get("status") != "operator_approval_request_not_authorization":
        raise SystemExit("approval request status must be operator_approval_request_not_authorization")
    if request.get("scope") != "REAL_FUNDS_CANARY":
        raise SystemExit("approval request scope must be REAL_FUNDS_CANARY")
    if request.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
        raise SystemExit("approval request execution_style must be GTC_LIMIT_POST_ONLY_CANCEL")
    if request.get("dual_control_required") is not True:
        raise SystemExit("approval request must require dual control")
    if request.get("live_submit_authorized") is not False:
        raise SystemExit("approval request must not itself authorize live submit")
    if request.get("remote_side_effects_authorized") is not False:
        raise SystemExit("approval request must not itself authorize remote side effects")
    if request.get("secrets_included") is not False:
        raise SystemExit("approval request must not include secrets")
    if not isinstance(request.get("active_profile_ref"), str) or not request["active_profile_ref"].strip():
        raise SystemExit("approval request active_profile_ref is required")
    operator_identity_ref = require_nonempty_text(
        request.get("operator_identity_ref"), "approval request operator_identity_ref"
    )
    operator_identity_sha256 = require_sha256(
        request.get("operator_identity_sha256"), "approval request operator_identity_sha256"
    )
    if operator_identity_sha256 != hashlib.sha256(operator_identity_ref.encode("utf-8")).hexdigest():
        raise SystemExit("approval request operator_identity_sha256 does not match operator_identity_ref")
    if not isinstance(request.get("condition_id"), str) or not request["condition_id"].strip():
        raise SystemExit("approval request condition_id is required")
    if parse_time(request.get("expires_at"), "approval request expires_at") <= datetime.now(timezone.utc):
        raise SystemExit("approval request is expired")
    for field in [
        "approval_hash",
        "artifact_sha256",
        "workspace_manifest_sha256",
        "archived_manifest_sha256",
        "evidence_manifest_sha256",
        "market_candidate_sha256",
        "runtime_truth_sha256",
    ]:
        require_sha256(request.get(field), f"approval request {field}")
    if request.get("archived_manifest_sha256") != request.get("evidence_manifest_sha256"):
        raise SystemExit("approval request archived/evidence manifest hashes must match")
    details = request.get("github_evidence_details")
    if not isinstance(details, dict):
        legacy_evidence = request.get("github_evidence")
        if not isinstance(legacy_evidence, dict):
            raise SystemExit("approval request github_evidence_details must be an object")
        details = legacy_github_evidence_details(legacy_evidence)
        request["github_evidence_details"] = details
    for section in ["root_ci", "hermes_ci", "execution_engine_ci", "credentialed_sdk"]:
        item = details.get(section)
        if not isinstance(item, dict):
            raise SystemExit(f"approval request github_evidence_details.{section} must be an object")
        for field in ["run_id", "workflow_name", "workflow_run_url", "commit_sha", "status", "timestamp"]:
            value = item.get(field)
            if not isinstance(value, str) or not value.strip() or value.startswith("REPLACE_WITH_"):
                raise SystemExit(f"approval request github_evidence_details.{section}.{field} must be concrete")
        if "://" not in item["workflow_run_url"]:
            raise SystemExit(f"approval request github_evidence_details.{section}.workflow_run_url must be a URL/ref")
        require_git_sha(item["commit_sha"], f"approval request github_evidence_details.{section}.commit_sha")
        if item["status"] not in {"success", "local_passed", "not_applicable_non_live"}:
            raise SystemExit(f"approval request github_evidence_details.{section}.status is invalid")
        parse_time(item["timestamp"], f"approval request github_evidence_details.{section}.timestamp")
    gate_snapshot = request.get("runtime_gate_snapshot")
    if not isinstance(gate_snapshot, dict):
        raise SystemExit("approval request runtime_gate_snapshot must be an object")
    for field in PREFLIGHT_GATE_FIELDS:
        if gate_snapshot.get(field) is not True:
            raise SystemExit(f"approval request runtime_gate_snapshot.{field} must be true")
    gate_evidence_refs = request.get("runtime_gate_evidence_refs")
    if not isinstance(gate_evidence_refs, dict):
        raise SystemExit("approval request runtime_gate_evidence_refs must be an object")
    for field in PREFLIGHT_GATE_FIELDS[2:]:
        value = gate_evidence_refs.get(field)
        if not isinstance(value, str) or not value.strip():
            raise SystemExit(f"approval request runtime_gate_evidence_refs.{field} must be a non-empty string")


def external_refs(
    external: dict[str, Any],
    *,
    dual_control_review_ref: str,
    dual_control_review_sha256: str | None = None,
) -> dict[str, str]:
    if has_placeholder(external):
        raise SystemExit("external references must not contain placeholders")
    refs: dict[str, str] = {}
    for output_key, path in REQUIRED_EXTERNAL_REFS.items():
        current: object = external
        for part in path:
            if not isinstance(current, dict):
                current = None
                break
            current = current.get(part)
        if not isinstance(current, str) or not current.strip():
            raise SystemExit(f"external reference missing {'.'.join(path)}")
        refs[output_key] = current
    refs["operator_dual_control_review_ref"] = dual_control_review_ref
    if dual_control_review_sha256 is not None:
        refs["operator_dual_control_review_sha256"] = dual_control_review_sha256
    return refs


def validate_dual_control_review(
    review: dict[str, Any],
    request: dict[str, Any],
    *,
    approval_request_sha256: str | None = None,
) -> str:
    if review.get("release_posture") != "non_live_hardened":
        raise SystemExit("dual-control review release_posture must be non_live_hardened")
    if review.get("schema_version") != 1:
        raise SystemExit("dual-control review schema_version must be 1")
    if review.get("status") != "approved":
        raise SystemExit("dual-control review status must be approved")
    if review.get("scope") != "REAL_FUNDS_CANARY":
        raise SystemExit("dual-control review scope must be REAL_FUNDS_CANARY")
    if review.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
        raise SystemExit("dual-control review execution_style must be GTC_LIMIT_POST_ONLY_CANCEL")
    if review.get("secrets_included") is not False:
        raise SystemExit("dual-control review must not include secrets")
    if parse_time(review.get("expires_at"), "dual-control review expires_at") <= datetime.now(timezone.utc):
        raise SystemExit("dual-control review is expired")
    reviewed_at = parse_time(review.get("reviewed_at"), "dual-control review reviewed_at")
    if reviewed_at > datetime.now(timezone.utc):
        raise SystemExit("dual-control review reviewed_at must not be in the future")

    if approval_request_sha256 is not None:
        reviewed_approval_sha = require_sha256(
            review.get("approval_request_sha256"),
            "dual-control review approval_request_sha256",
        )
        if reviewed_approval_sha != approval_request_sha256:
            raise SystemExit("dual-control review approval_request_sha256 does not match approval request file")

    reviewer = review.get("reviewer_identity_ref")
    if not isinstance(reviewer, str) or not reviewer.strip() or reviewer.startswith("REPLACE_WITH_"):
        raise SystemExit("dual-control review reviewer_identity_ref is required")
    reviewer_identity_sha256 = require_sha256(
        review.get("reviewer_identity_sha256"), "dual-control review reviewer_identity_sha256"
    )
    if reviewer_identity_sha256 != hashlib.sha256(reviewer.encode("utf-8")).hexdigest():
        raise SystemExit("dual-control review reviewer_identity_sha256 does not match reviewer_identity_ref")
    if reviewer == request.get("operator_identity_ref"):
        raise SystemExit("dual-control reviewer must differ from operator_identity_ref")

    review_ref = review.get("review_ref")
    if not isinstance(review_ref, str) or not review_ref.strip() or review_ref.startswith("REPLACE_WITH_"):
        raise SystemExit("dual-control review_ref is required")

    for field in [
        "approval_hash",
        "artifact_sha256",
        "workspace_manifest_sha256",
        "archived_manifest_sha256",
        "evidence_manifest_sha256",
        "market_candidate_sha256",
        "runtime_truth_sha256",
    ]:
        reviewed_value = require_sha256(review.get(field), f"dual-control review {field}")
        requested_value = require_sha256(request.get(field), f"approval request {field}")
        if reviewed_value != requested_value:
            raise SystemExit(f"dual-control review {field} does not match approval request")

    request_limits = request.get("risk_limits", {})
    review_limits = review.get("risk_limits", {})
    if not isinstance(request_limits, dict) or not isinstance(review_limits, dict):
        raise SystemExit("dual-control review risk_limits must be an object")
    for field in ["max_order_notional_usd", "max_daily_notional_usd"]:
        if review_limits.get(field) != request_limits.get(field):
            raise SystemExit(f"dual-control review risk_limits.{field} does not match approval request")
    checks = review.get("required_reviewer_checks")
    if not isinstance(checks, dict):
        raise SystemExit("dual-control review required_reviewer_checks must be an object")
    missing_checks = [key for key in REQUIRED_DUAL_CONTROL_CHECKS if checks.get(key) is not True]
    if missing_checks:
        raise SystemExit("dual-control review missing required reviewer checks: " + ", ".join(missing_checks))
    return review_ref


def build_decision(
    request: dict[str, Any],
    external: dict[str, Any],
    *,
    decision_id: str,
    decision_reason: str,
    dual_control_review: dict[str, Any],
    dual_control_review_sha256: str | None = None,
    approval_request_sha256: str | None = None,
) -> dict[str, Any]:
    validate_approval_request(request)
    resolved_decision_id = require_decision_id(decision_id)
    resolved_reason = require_concrete_text(decision_reason, "decision_reason")
    dual_control_review_ref = validate_dual_control_review(
        dual_control_review,
        request,
        approval_request_sha256=approval_request_sha256,
    )
    refs = external_refs(
        external,
        dual_control_review_ref=dual_control_review_ref,
        dual_control_review_sha256=dual_control_review_sha256,
    )
    review_checks = dual_control_review["required_reviewer_checks"]
    review_signals = {
        "artifact_hash_reviewed": review_checks["artifact_hash_reviewed"] is True,
        "evidence_manifest_hash_reviewed": review_checks["evidence_manifest_hash_reviewed"] is True,
        "market_candidate_reviewed": review_checks["market_candidate_reviewed"] is True,
        "operator_dual_control_reviewed": bool(dual_control_review_ref.strip()),
        "secret_custody_reviewed": review_checks["secret_custody_reviewed"] is True and bool(refs["secret_custody_ref"].strip()),
        "alerting_reviewed": review_checks["alerting_reviewed"] is True and bool(refs["alert_routing_ref"].strip()) and bool(refs["dashboard_ref"].strip()),
        "rollback_reviewed": review_checks["rollback_reviewed"] is True and bool(refs["rollback_runbook_ref"].strip()) and bool(refs["incident_runbook_ref"].strip()),
        "runtime_health_reviewed": review_checks["runtime_truth_reviewed"] is True,
        "reconcile_and_cancel_fallback_reviewed": review_checks["reconcile_and_cancel_fallback_reviewed"] is True,
    }
    return {
        "schema_version": 1,
        "release_posture": "non_live_hardened",
        "decision_id": resolved_decision_id,
        "status": "reviewed_go",
        "source_release": f"v{VERSION}",
        "decision": "go",
        "decision_reason": resolved_reason,
        "scope": "REAL_FUNDS_CANARY",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "expires_at": request["expires_at"],
        "artifact_sha256": request["artifact_sha256"],
        "evidence_manifest_sha256": request["evidence_manifest_sha256"],
        "workspace_manifest_sha256": request["workspace_manifest_sha256"],
        "archived_manifest_sha256": request["archived_manifest_sha256"],
        "market_candidate_sha256": request["market_candidate_sha256"],
        "condition_id": request["condition_id"],
        "github_evidence": request["github_evidence"],
        "github_evidence_details": request["github_evidence_details"],
        "external_references": refs,
        "risk_limits": {
            "max_order_notional_usd": request["risk_limits"]["max_order_notional_usd"],
            "max_daily_notional_usd": request["risk_limits"]["max_daily_notional_usd"],
        },
        "runtime_gate_snapshot": request["runtime_gate_snapshot"],
        "runtime_gate_evidence_refs": request["runtime_gate_evidence_refs"],
        "required_review_signals": review_signals,
        "live_submit_authorized": True,
        "live_cancel_authorized": True,
        "production_deployment_authorized": False,
        "real_funds_canary_authorized": True,
        "remote_side_effects_authorized": True,
        "single_attempt": True,
        "max_order_count": 1,
        "post_cancel_required": True,
        "readback_closeout_required": True,
        "allow_real_funds_canary": True,
        "reviewed_release_decision_present": True,
        "operator_identity_ref": request["operator_identity_ref"],
        "operator_identity_sha256": request["operator_identity_sha256"],
        "reviewer_identity_sha256": dual_control_review["reviewer_identity_sha256"],
        "secrets_included": False,
    }


def validate_decision_output(decision: dict[str, Any]) -> None:
    spec = importlib.util.spec_from_file_location("validate_controlled_canary_release_decision", VALIDATOR)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    failures = module.validate_decision(decision, "reviewed_go_output")
    if failures:
        raise SystemExit("reviewed-go decision validation failed: " + "; ".join(failures))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--approval-request-file", required=True, type=Path)
    parser.add_argument("--external-references-file", required=True, type=Path)
    parser.add_argument("--dual-control-review-file", required=True, type=Path)
    parser.add_argument("--decision-id", required=True)
    parser.add_argument("--decision-reason", required=True)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    approval_path = args.approval_request_file if args.approval_request_file.is_absolute() else ROOT / args.approval_request_file
    external_path = args.external_references_file if args.external_references_file.is_absolute() else ROOT / args.external_references_file
    review_path = args.dual_control_review_file if args.dual_control_review_file.is_absolute() else ROOT / args.dual_control_review_file
    output = args.output if args.output.is_absolute() else ROOT / args.output
    decision = build_decision(
        load_json(approval_path),
        load_json(external_path),
        decision_id=args.decision_id,
        decision_reason=args.decision_reason,
        dual_control_review=load_json(review_path),
        dual_control_review_sha256=require_sha256(sha256(review_path), "dual-control review file sha256"),
        approval_request_sha256=require_sha256(sha256(approval_path), "approval request file sha256"),
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    validate_decision_output(decision)
    output.write_text(json.dumps(decision, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": "pass", "output": str(output)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
