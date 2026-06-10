#!/usr/bin/env python3
"""Shared helper layer for controlled-canary approval request assembly."""
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from datetime import datetime, timedelta, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
VERSION = (INTEGRATION_ROOT / "VERSION").read_text().strip()
DEFAULT_RELEASE_ZIP = INTEGRATION_ROOT / "dist" / f"polymarket-execution-suite-v{VERSION}.zip"
ACTIVE_PROFILE_CHECK = ROOT / "validation" / "check_active_profile_consistency.py"
PREFLIGHT_GATE_FIELDS = (
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
)
PREFLIGHT_GATE_EVIDENCE_FIELDS = (
    "kill_switch_open",
    "runtime_worker_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_worker_healthy",
    "cancel_only_fallback_ready",
    "balance_allowance_checked",
)
IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def load_module(path: Path, name: str):
    import importlib.util
    import sys

    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def require_sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(ch not in "0123456789abcdefABCDEF" for ch in value):
        raise SystemExit(f"{label} must be a 64-character SHA-256 hex digest")
    return value.lower()


def require_nonempty_text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip() or value.startswith("REPLACE_WITH_"):
        raise SystemExit(f"{label} must be a non-empty non-placeholder string")
    return value.strip()


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


def decimal_value(value: object, label: str) -> Decimal:
    try:
        parsed = Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise SystemExit(f"{label} must be a decimal") from exc
    if not parsed.is_finite() or parsed <= 0:
        raise SystemExit(f"{label} must be positive")
    return parsed


def decimal_text(value: Decimal) -> str:
    text = format(value.normalize(), "f")
    return "0" if text == "-0" else text


def git_head(repo: Path) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        cwd=INTEGRATION_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise SystemExit(f"failed to resolve git HEAD for {repo}: {completed.stderr.strip()}")
    return completed.stdout.strip()


def github_evidence_details(
    *,
    root_ci_run_id: str,
    hermes_ci_run_id: str,
    execution_engine_ci_run_id: str,
    credentialed_sdk_run_id: str,
    timestamp: datetime,
) -> dict[str, dict[str, str]]:
    root_head = git_head(INTEGRATION_ROOT)
    hermes_head = git_head(INTEGRATION_ROOT / "hermes-polymarket-executor-adapter")
    engine_head = git_head(INTEGRATION_ROOT / "polymarket-execution-engine")
    return {
        "root_ci": {
            "run_id": require_nonempty_text(root_ci_run_id, "root_ci_run_id"),
            "workflow_name": "ci",
            "workflow_run_url": f"https://github.com/ray-toaru/polymarket_dual_project/actions/runs/{root_ci_run_id}",
            "commit_sha": root_head,
            "status": "success",
            "timestamp": timestamp.isoformat(),
        },
        "hermes_ci": {
            "run_id": require_nonempty_text(hermes_ci_run_id, "hermes_ci_run_id"),
            "workflow_name": "ci",
            "workflow_run_url": f"https://github.com/ray-toaru/hermes-polymarket-executor-adapter/actions/runs/{hermes_ci_run_id}",
            "commit_sha": hermes_head,
            "status": "success",
            "timestamp": timestamp.isoformat(),
        },
        "execution_engine_ci": {
            "run_id": require_nonempty_text(execution_engine_ci_run_id, "execution_engine_ci_run_id"),
            "workflow_name": "ci",
            "workflow_run_url": f"https://github.com/ray-toaru/polymarket-execution-engine/actions/runs/{execution_engine_ci_run_id}",
            "commit_sha": engine_head,
            "status": "success",
            "timestamp": timestamp.isoformat(),
        },
        "credentialed_sdk": {
            "run_id": require_nonempty_text(credentialed_sdk_run_id, "credentialed_sdk_run_id"),
            "workflow_name": "credentialed-sdk-local-non-live",
            "workflow_run_url": "local-evidence://polymarket-execution-engine/evidence/current/credentialed-sdk",
            "commit_sha": engine_head,
            "status": "local_passed",
            "timestamp": timestamp.isoformat(),
        },
    }


def validate_runtime_truth(
    runtime_truth: dict[str, Any],
    *,
    expected_account_id: str,
) -> dict[str, Any]:
    account_id = require_nonempty_text(runtime_truth.get("account_id"), "runtime truth account_id")
    if account_id != expected_account_id:
        raise SystemExit("runtime truth account_id does not match active runtime account")
    condition_id = require_nonempty_text(runtime_truth.get("condition_id"), "runtime truth condition_id")
    preflight_report = runtime_truth.get("preflight_report")
    if not isinstance(preflight_report, dict):
        raise SystemExit("runtime truth preflight_report must be an object")
    if preflight_report.get("status") != "preflight_ready":
        raise SystemExit("runtime truth preflight_report.status must be preflight_ready")
    gate_snapshot: dict[str, bool] = {}
    for field in PREFLIGHT_GATE_FIELDS:
        if preflight_report.get(field) is not True:
            raise SystemExit(f"runtime truth preflight_report.{field} must be true")
        gate_snapshot[field] = True
    gate_evidence_refs = preflight_report.get("gate_evidence_refs")
    if not isinstance(gate_evidence_refs, dict):
        raise SystemExit("runtime truth preflight_report.gate_evidence_refs must be an object")
    validated_gate_evidence_refs: dict[str, str] = {}
    for field in PREFLIGHT_GATE_EVIDENCE_FIELDS:
        evidence_ref = gate_evidence_refs.get(field)
        if not isinstance(evidence_ref, str) or not evidence_ref.strip() or "REPLACE_WITH" in evidence_ref:
            raise SystemExit(f"runtime truth preflight_report.gate_evidence_refs.{field} must be a concrete string")
        validated_gate_evidence_refs[field] = evidence_ref.strip()
    return {
        "account_id": account_id,
        "condition_id": condition_id,
        "gate_snapshot": gate_snapshot,
        "gate_evidence_refs": validated_gate_evidence_refs,
    }


def resolve_runtime_identity(
    *,
    runtime_env_file: Path | None,
    account_id: str | None,
    active_profile_ref: str | None,
) -> tuple[str, str]:
    resolved_account = account_id.strip() if isinstance(account_id, str) and account_id.strip() else None
    resolved_profile_ref = (
        active_profile_ref.strip()
        if isinstance(active_profile_ref, str) and active_profile_ref.strip()
        else None
    )
    if runtime_env_file is not None:
        checker = load_module(ACTIVE_PROFILE_CHECK, "check_active_profile_consistency")
        report = checker.evaluate_identity_env_file(
            runtime_env_file,
            expected_account_id=resolved_account,
        )
        env_account = require_nonempty_text(report.get("active_account_id"), "runtime env active_account_id")
        env_profile_ref = require_nonempty_text(report.get("active_profile_ref"), "runtime env active_profile_ref")
        if resolved_account is not None and resolved_account != env_account:
            raise SystemExit("runtime env account_id does not match explicit --account-id")
        if resolved_profile_ref is not None and resolved_profile_ref != env_profile_ref:
            raise SystemExit("runtime env active_profile_ref does not match explicit --active-profile-ref")
        return env_account, env_profile_ref
    if resolved_account is None:
        raise SystemExit("either --account-id or --runtime-env-file is required")
    if resolved_profile_ref is None:
        raise SystemExit("either --active-profile-ref or --runtime-env-file is required")
    return resolved_account, resolved_profile_ref


def build_request(
    *,
    account_id: str,
    condition_id: str,
    active_profile_ref: str,
    operator_identity_ref: str,
    approval_ticket_ref: str,
    candidate_market_file: Path,
    runtime_truth_file: Path,
    runtime_gate_snapshot: dict[str, bool],
    runtime_gate_evidence_refs: dict[str, str],
    sidecar: dict[str, str],
    candidate_limits: dict[str, str],
    max_order_notional: Decimal,
    max_daily_notional: Decimal,
    root_ci_run_id: str,
    hermes_ci_run_id: str,
    execution_engine_ci_run_id: str,
    credentialed_sdk_run_id: str,
    valid_for_minutes: int,
) -> dict[str, Any]:
    now = datetime.now(timezone.utc)
    request = {
        "schema_version": 1,
        "release_posture": "non_live_hardened",
        "status": "operator_approval_request_not_authorization",
        "approval_id": f"approval-request-controlled-canary-{now.strftime('%Y%m%dT%H%M%SZ')}",
        "scope": "REAL_FUNDS_CANARY",
        "account_id": account_id,
        "condition_id": condition_id,
        "active_profile_ref": active_profile_ref,
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "requested_at": now.isoformat(),
        "expires_at": (now + timedelta(minutes=valid_for_minutes)).isoformat(),
        "operator_identity_ref": require_nonempty_text(operator_identity_ref, "operator_identity_ref"),
        "operator_identity_sha256": hashlib.sha256(
            require_nonempty_text(operator_identity_ref, "operator_identity_ref").encode("utf-8")
        ).hexdigest(),
        "approval_ticket_ref": require_nonempty_text(approval_ticket_ref, "approval_ticket_ref"),
        "dual_control_required": True,
        "approval_replay_block_required": True,
        "approval_expiry_enforced": True,
        "artifact_sha256": sidecar["artifact_sha256"],
        "workspace_manifest_sha256": sidecar["workspace_manifest_sha256"],
        "archived_manifest_sha256": sidecar["archived_manifest_sha256"],
        "evidence_manifest_sha256": sidecar["evidence_manifest_sha256"],
        "market_candidate_sha256": sha256(candidate_market_file),
        "runtime_truth_sha256": sha256(runtime_truth_file),
        "runtime_gate_snapshot": runtime_gate_snapshot,
        "runtime_gate_evidence_refs": runtime_gate_evidence_refs,
        "github_evidence": {
            "root_ci_run_id": root_ci_run_id,
            "hermes_ci_run_id": hermes_ci_run_id,
            "execution_engine_ci_run_id": execution_engine_ci_run_id,
            "credentialed_sdk_run_id": credentialed_sdk_run_id,
        },
        "github_evidence_details": github_evidence_details(
            root_ci_run_id=root_ci_run_id,
            hermes_ci_run_id=hermes_ci_run_id,
            execution_engine_ci_run_id=execution_engine_ci_run_id,
            credentialed_sdk_run_id=credentialed_sdk_run_id,
            timestamp=now,
        ),
        "risk_limits": {
            "max_order_notional_usd": decimal_text(max_order_notional),
            "max_daily_notional_usd": decimal_text(max_daily_notional),
            "candidate_target_size": candidate_limits["target_size"],
            "candidate_limit_price": candidate_limits["limit_price"],
            "candidate_estimated_order_notional_usd": candidate_limits["estimated_order_notional_usd"],
        },
        "live_submit_authorized": False,
        "live_cancel_authorized": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects_authorized": False,
        "production_deployment_authorized": False,
        "secrets_included": False,
    }
    request["approval_hash"] = compute_approval_hash(request)
    return request


def load_release_sidecar(release_zip: Path) -> dict[str, Any]:
    sidecar = release_zip.with_suffix(release_zip.suffix + ".evidence.json")
    if not sidecar.exists():
        raise SystemExit(f"release evidence sidecar missing: {sidecar}")
    data = load_json(sidecar)
    artifact = data.get("artifact", {})
    canonical = data.get("canonical_evidence", {})
    return {
        "artifact_sha256": require_sha256(artifact.get("sha256"), "artifact.sha256"),
        "workspace_manifest_sha256": require_sha256(
            canonical.get("workspace_manifest_sha256"),
            "canonical_evidence.workspace_manifest_sha256",
        ),
        "archived_manifest_sha256": require_sha256(
            canonical.get("archived_manifest_sha256"),
            "canonical_evidence.archived_manifest_sha256",
        ),
        "evidence_manifest_sha256": require_sha256(
            canonical.get("archived_manifest_sha256"),
            "canonical_evidence.archived_manifest_sha256",
        ),
    }


def validate_candidate(candidate: dict[str, Any], max_order_notional: Decimal) -> dict[str, str]:
    market_id = require_nonempty_text(candidate.get("market_id"), "candidate market_id")
    if not IDENTIFIER_RE.fullmatch(market_id):
        raise SystemExit("candidate market_id must use a stable identifier format")
    if candidate.get("side") != "BUY":
        raise SystemExit("candidate side must be BUY")
    if candidate.get("order_type") != "GTC":
        raise SystemExit("candidate order_type must be GTC")
    if candidate.get("post_only") is not True:
        raise SystemExit("candidate post_only must be true")
    if candidate.get("active") is not True:
        raise SystemExit("candidate active must be true")
    if candidate.get("accepting_orders") is not True:
        raise SystemExit("candidate accepting_orders must be true")
    if candidate.get("closed") is not False:
        raise SystemExit("candidate closed must be false")
    if candidate.get("archived") is not False:
        raise SystemExit("candidate archived must be false")
    target_size = decimal_value(candidate.get("target_size"), "candidate target_size")
    limit_price = decimal_value(candidate.get("limit_price"), "candidate limit_price")
    notional = decimal_value(candidate.get("estimated_order_notional_usd"), "candidate estimated_order_notional_usd")
    if notional != target_size * limit_price:
        raise SystemExit("candidate estimated_order_notional_usd must equal target_size * limit_price")
    if notional > max_order_notional:
        raise SystemExit("candidate notional exceeds requested max_order_notional_usd")
    snapshot = candidate.get("exchange_rule_snapshot")
    if not isinstance(snapshot, dict):
        raise SystemExit("candidate exchange_rule_snapshot is required")
    book_snapshot_timestamp = parse_time(
        candidate.get("book_snapshot_timestamp"), "candidate book_snapshot_timestamp"
    )
    captured_at = parse_time(snapshot.get("captured_at"), "candidate exchange_rule_snapshot.captured_at")
    parsed_expiry = parse_time(snapshot.get("expires_at"), "candidate exchange_rule_snapshot.expires_at")
    if captured_at != book_snapshot_timestamp:
        raise SystemExit("candidate exchange_rule_snapshot.captured_at must equal book_snapshot_timestamp")
    if parsed_expiry <= captured_at:
        raise SystemExit("candidate exchange_rule_snapshot.expires_at must be after captured_at")
    now = datetime.now(timezone.utc)
    if book_snapshot_timestamp > now:
        raise SystemExit("candidate book_snapshot_timestamp must not be in the future")
    if parsed_expiry <= now:
        raise SystemExit("candidate exchange_rule_snapshot.expires_at must be in the future")
    return {
        "market_id": market_id,
        "target_size": decimal_text(target_size),
        "limit_price": decimal_text(limit_price),
        "estimated_order_notional_usd": decimal_text(notional),
    }


def canonical_approval_payload(request: dict[str, Any]) -> dict[str, Any]:
    payload = dict(request)
    payload.pop("approval_hash", None)
    return payload


def compute_approval_hash(request: dict[str, Any]) -> str:
    payload = canonical_approval_payload(request)
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()
