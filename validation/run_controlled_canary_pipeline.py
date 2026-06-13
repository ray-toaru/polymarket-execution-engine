#!/usr/bin/env python3
"""Run the fail-closed controlled canary preparation pipeline.

This script intentionally stops at a no-go blocked rehearsal. It never creates a
reviewed-go decision, never submits an order, and never cancels a live order.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
ENGINE = ROOT
VERSION = (INTEGRATION_ROOT / "VERSION").read_text().strip()
DEFAULT_RELEASE_ZIP = INTEGRATION_ROOT / "dist" / f"polymarket-execution-suite-v{VERSION}.zip"
DEFAULT_MANIFEST = ENGINE / "evidence" / "current" / "manifest.json"
DEFAULT_EXTERNAL_REFERENCES = ENGINE / "config" / "controlled-canary.external-references.example.json"
BLOCKED_REHEARSAL = ENGINE / "validation" / "run_real_funds_canary_blocked_rehearsal_package.py"
PREPARE_CANDIDATE = ENGINE / "validation" / "prepare_canary_candidate_market.py"
PREPARE_CLOSEOUT = INTEGRATION_ROOT / "scripts" / "prepare_canary_closeout.py"
VALIDATE_RUNTIME_TRUTH = ENGINE / "validation" / "validate_controlled_canary_runtime_truth.py"
MAX_PRICE = Decimal("1")


def parse_decimal(value: Any, field: str) -> Decimal:
    try:
        decimal = Decimal(str(value))
    except (InvalidOperation, ValueError):
        raise SystemExit(f"candidate {field} must be a decimal")
    if not decimal.is_finite():
        raise SystemExit(f"candidate {field} must be finite")
    return decimal


def require_text(data: dict[str, Any], field: str) -> str:
    value = data.get(field)
    if not isinstance(value, str) or not value.strip() or "REPLACE_WITH" in value:
        raise SystemExit(f"candidate {field} is required")
    return value.strip()


def require_bool(data: dict[str, Any], field: str, expected: bool) -> None:
    if data.get(field) is not expected:
        raise SystemExit(f"candidate {field} must be {str(expected).lower()}")


def validate_candidate_file(
    path: Path,
    *,
    max_order_notional_usd: Decimal | None = None,
    max_spread_bps: int | None = None,
) -> dict[str, Any]:
    candidate = load_json(path)
    require_text(candidate, "market_id")
    require_text(candidate, "token_id")
    require_text(candidate, "human_review_ref")
    if candidate.get("side") != "BUY":
        raise SystemExit("candidate side must be BUY")
    if candidate.get("order_type") != "GTC":
        raise SystemExit("candidate order_type must be GTC")
    require_bool(candidate, "post_only", True)
    require_bool(candidate, "active", True)
    require_bool(candidate, "accepting_orders", True)
    require_bool(candidate, "closed", False)
    require_bool(candidate, "archived", False)

    best_ask = parse_decimal(candidate.get("best_ask"), "best_ask")
    limit_price = parse_decimal(candidate.get("limit_price"), "limit_price")
    ask_size = parse_decimal(candidate.get("ask_size"), "ask_size")
    target_size = parse_decimal(candidate.get("target_size"), "target_size")
    min_order_size = parse_decimal(candidate.get("min_order_size"), "min_order_size")
    spread_bps = candidate.get("spread_bps")
    estimated_notional = parse_decimal(
        candidate.get("estimated_order_notional_usd"),
        "estimated_order_notional_usd",
    )
    if best_ask <= 0 or limit_price <= 0 or ask_size <= 0 or target_size <= 0:
        raise SystemExit("candidate best_ask, limit_price, ask_size, and target_size must be positive")
    if best_ask > MAX_PRICE or limit_price > MAX_PRICE:
        raise SystemExit("candidate best_ask and limit_price must be within Polymarket price bounds")
    if estimated_notional != limit_price * target_size:
        raise SystemExit("candidate estimated_order_notional_usd must equal limit_price * target_size")
    if max_order_notional_usd is not None and estimated_notional > max_order_notional_usd:
        raise SystemExit("candidate estimated_order_notional_usd exceeds max_order_notional_usd")
    if limit_price >= best_ask:
        raise SystemExit("candidate post-only BUY limit_price must be below best_ask")
    if ask_size < target_size:
        raise SystemExit("candidate ask_size must cover target_size")
    if min_order_size > target_size:
        raise SystemExit("candidate min_order_size must not exceed target_size")
    if not isinstance(spread_bps, int) or spread_bps < 0:
        raise SystemExit("candidate spread_bps must be a non-negative integer")
    if max_spread_bps is not None and spread_bps > max_spread_bps:
        raise SystemExit("candidate spread_bps exceeds max_spread_bps")

    snapshot = candidate.get("exchange_rule_snapshot")
    if not isinstance(snapshot, dict):
        raise SystemExit("candidate exchange_rule_snapshot is required")
    required_snapshot = {
        "schema_version": 1,
        "venue": "polymarket_clob",
        "order_mode": "post_only_limit",
        "order_type": "GTC",
        "side": "BUY",
        "target_size_semantics": "outcome_shares",
    }
    for field, expected in required_snapshot.items():
        if snapshot.get(field) != expected:
            raise SystemExit(f"candidate exchange_rule_snapshot.{field} must be {expected!r}")
    require_text(snapshot, "source")
    require_text(snapshot, "evidence_ref")
    min_share_size = parse_decimal(snapshot.get("min_share_size"), "exchange_rule_snapshot.min_share_size")
    min_tick_size = parse_decimal(snapshot.get("min_tick_size"), "exchange_rule_snapshot.min_tick_size")
    if min_share_size <= 0 or min_tick_size <= 0:
        raise SystemExit("candidate exchange_rule_snapshot min_share_size and min_tick_size must be positive")
    if min_share_size > target_size:
        raise SystemExit("candidate exchange_rule_snapshot.min_share_size must not exceed target_size")
    if (limit_price / min_tick_size) % 1 != 0:
        raise SystemExit("candidate limit_price must align with exchange_rule_snapshot.min_tick_size")
    parsed_times: dict[str, datetime] = {}
    for field in ("captured_at", "expires_at"):
        value = require_text(snapshot, field)
        try:
            parsed_times[field] = datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)
        except ValueError:
            raise SystemExit(f"candidate exchange_rule_snapshot.{field} must be RFC3339")
    captured_at = parsed_times["captured_at"]
    expires_at = parsed_times["expires_at"]
    now = datetime.now(timezone.utc)
    if captured_at > now:
        raise SystemExit("candidate exchange_rule_snapshot.captured_at must not be in the future")
    if expires_at <= captured_at:
        raise SystemExit("candidate exchange_rule_snapshot.expires_at must be after captured_at")
    if expires_at <= now:
        raise SystemExit("candidate exchange_rule_snapshot.expires_at must be in the future")
    return candidate


def build_stage_plan(
    *,
    reviewed_go: bool,
    closeout_package_dir: Path | None,
    runtime_truth_ready: bool,
) -> list[dict[str, Any]]:
    armed_status = "blocked"
    armed_side_effects = False
    if reviewed_go and runtime_truth_ready:
        armed_status = "requires_explicit_operator_run"
        armed_side_effects = False
    elif reviewed_go:
        armed_status = "blocked_runtime_truth_missing"
    readback_status = "required_after_armed_run" if reviewed_go and runtime_truth_ready else "blocked"
    return [
        {"stage": "candidate", "status": "required", "remote_side_effects": False},
        {"stage": "no_go_review", "status": "required", "remote_side_effects": False},
        {"stage": "blocked_rehearsal", "status": "required", "remote_side_effects": False},
        {
            "stage": "reviewed_go_decision",
            "status": "provided" if reviewed_go else "blocked",
            "remote_side_effects": False,
        },
        {
            "stage": "armed_post_cancel",
            "status": armed_status,
            "remote_side_effects": armed_side_effects,
            "operator_run_would_have_remote_side_effects": reviewed_go and runtime_truth_ready,
        },
        {
            "stage": "readback",
            "status": readback_status,
            "remote_side_effects": False,
        },
        {
            "stage": "closeout",
            "status": "available" if closeout_package_dir else "blocked",
            "remote_side_effects": False,
        },
    ]


def build_operator_runbook(
    *,
    reviewed_go: bool,
    runtime_truth_ready: bool,
    reviewed_go_decision: dict[str, Any] | None,
) -> dict[str, Any]:
    enabled = reviewed_go and runtime_truth_ready and reviewed_go_decision is not None
    steps = [
        {
            "stage": "preflight",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "fresh reviewed-go decision and durable runtime truth are still valid",
        },
        {
            "stage": "armed_post_cancel",
            "remote_side_effects": True,
            "required": True,
            "success_condition": "exactly one post-only BUY/GTC order is posted and cancel is attempted immediately",
        },
        {
            "stage": "readback_order",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "remote order status is cancelled or escalated operator-required",
        },
        {
            "stage": "readback_trades",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "trade query is persisted even when zero fills are observed",
        },
        {
            "stage": "readback_account_activity",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "account activity and position readback are persisted",
        },
        {
            "stage": "closeout",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "closeout.json and CLOSEOUT.md are generated for the exact package",
        },
        {
            "stage": "mark_consumed",
            "remote_side_effects": False,
            "required": True,
            "success_condition": "approval-consumed marker prevents reuse",
        },
    ]
    return {
        "schema_version": 1,
        "status": "operator_runnable_not_auto_executed" if enabled else "blocked",
        "auto_execute": False,
        "requires_fresh_reviewed_go": True,
        "requires_runtime_truth": True,
        "requires_closeout": True,
        "reviewed_go_decision": reviewed_go_decision,
        "steps": steps,
        "blocked_reason": None if enabled else "reviewed-go decision and durable runtime truth are required",
    }


def runtime_truth_dependencies() -> list[dict[str, Any]]:
    return [
        {
            "name": "kill_switch",
            "required_before_live": True,
            "current_pipeline_state": "local_evidence_only",
            "required_truth": "durable runtime state checked before post and cancel",
        },
        {
            "name": "live_submit_gate",
            "required_before_live": True,
            "current_pipeline_state": "release_decision_only",
            "required_truth": "runtime gate evaluated immediately before remote side effect",
        },
        {
            "name": "idempotency_lease",
            "required_before_live": True,
            "current_pipeline_state": "local_package_binding_only",
            "required_truth": "leased owner token with recovery for remote-unknown attempts",
        },
        {
            "name": "order_cancel_reconciliation",
            "required_before_live": True,
            "current_pipeline_state": "closeout_evidence_only",
            "required_truth": "durable order/cancel state machine with operator-required escalation",
        },
    ]


def required_runtime_truth_names() -> set[str]:
    return {item["name"] for item in runtime_truth_dependencies()}


def validate_runtime_truth_file(
    path: Path,
    *,
    expected_account_id: str | None = None,
    expected_artifact_sha256: str | None = None,
    expected_workspace_manifest_sha256: str | None = None,
    expected_archived_manifest_sha256: str | None = None,
) -> dict[str, Any]:
    completed = subprocess.run(
        [sys.executable, str(VALIDATE_RUNTIME_TRUTH), "--file", str(path)],
        cwd=ENGINE,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise SystemExit(f"runtime truth validator failed: {detail}")
    data = load_json(path)
    expected_fields = {
        "account_id": expected_account_id,
        "artifact_sha256": expected_artifact_sha256,
        "workspace_manifest_sha256": expected_workspace_manifest_sha256,
        "archived_manifest_sha256": expected_archived_manifest_sha256,
    }
    mismatched = [
        f"{field} expected {expected}, got {data.get(field)!r}"
        for field, expected in expected_fields.items()
        if expected is not None and data.get(field) != expected
    ]
    if mismatched:
        raise SystemExit("runtime truth artifact binding mismatch: " + "; ".join(mismatched))
    if data.get("schema_version") != 1:
        raise SystemExit("runtime truth schema_version must be 1")
    dependencies = data.get("dependencies")
    if not isinstance(dependencies, list):
        raise SystemExit("runtime truth dependencies must be a list")
    ready: dict[str, dict[str, Any]] = {}
    for item in dependencies:
        if not isinstance(item, dict):
            continue
        name = item.get("name")
        status = item.get("status")
        evidence_ref = item.get("evidence_ref")
        if (
            isinstance(name, str)
            and status == "durable_runtime_truth"
            and isinstance(evidence_ref, str)
            and evidence_ref.strip()
            and "REPLACE_WITH" not in evidence_ref
        ):
            ready[name] = item
    missing = sorted(required_runtime_truth_names() - set(ready))
    if missing:
        raise SystemExit("runtime truth missing durable dependencies: " + ", ".join(missing))
    report = data.get("preflight_report")
    if not isinstance(report, dict):
        raise SystemExit("runtime truth preflight_report must be an object")
    return {
        "schema_version": 1,
        "ready_for_armed_stage": True,
        "path": str(path),
        "sha256": sha256(path),
        "account_id": data.get("account_id"),
        "artifact_sha256": data.get("artifact_sha256"),
        "workspace_manifest_sha256": data.get("workspace_manifest_sha256"),
        "archived_manifest_sha256": data.get("archived_manifest_sha256"),
        "preflight_report": report,
        "dependencies": [ready[name] for name in sorted(ready)],
    }


def validate_reviewed_go_decision_file(path: Path) -> dict[str, Any]:
    consumed_markers = sorted(path.parent.glob("approval-consumed*.json"))
    if consumed_markers:
        raise SystemExit(
            "reviewed-go decision package already consumed: "
            + ", ".join(marker.name for marker in consumed_markers)
        )
    if (path.parent / "closeout.json").exists() or (path.parent / "CLOSEOUT.md").exists():
        raise SystemExit("reviewed-go decision package already closed; fresh decision required")

    decision = load_json(path)
    required = [
        (decision.get("decision") == "go", "decision must be go"),
        (decision.get("status") == "reviewed_go", "status must be reviewed_go"),
        (decision.get("scope") == "REAL_FUNDS_CANARY", "scope must be REAL_FUNDS_CANARY"),
        (
            decision.get("execution_style") == "GTC_LIMIT_POST_ONLY_CANCEL",
            "execution_style must be GTC_LIMIT_POST_ONLY_CANCEL",
        ),
        (decision.get("live_submit_authorized") is True, "live_submit_authorized must be true"),
        (decision.get("live_cancel_authorized") is True, "live_cancel_authorized must be true"),
        (
            decision.get("real_funds_canary_authorized") is True,
            "real_funds_canary_authorized must be true",
        ),
        (
            decision.get("remote_side_effects_authorized") is True,
            "remote_side_effects_authorized must be true",
        ),
        (
            decision.get("production_deployment_authorized") is False,
            "production_deployment_authorized must remain false",
        ),
        (decision.get("single_attempt") is True, "single_attempt must be true"),
        (decision.get("max_order_count") == 1, "max_order_count must be 1"),
        (
            decision.get("post_cancel_required") is True,
            "post_cancel_required must be true",
        ),
        (
            decision.get("readback_closeout_required") is True,
            "readback_closeout_required must be true",
        ),
    ]
    failures = [reason for ok, reason in required if not ok]
    if failures:
        raise SystemExit("reviewed-go decision invalid: " + "; ".join(failures))
    return {
        "path": str(path),
        "sha256": sha256(path),
        "decision_id": decision.get("decision_id"),
        "single_attempt": True,
        "max_order_count": 1,
    }


def run_closeout_stage(package_dir: Path, release_zip: Path) -> dict[str, Any]:
    command = [
        sys.executable,
        str(PREPARE_CLOSEOUT),
        "--package-dir",
        str(package_dir),
        "--release-zip",
        str(release_zip),
    ]
    completed = run(command, cwd=INTEGRATION_ROOT)
    stage = {
        "stage": "closeout",
        "status": "pass" if completed.returncode == 0 else "fail",
        "exit_code": completed.returncode,
        "package_dir": str(package_dir),
        "remote_side_effects": False,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }
    if completed.returncode != 0:
        raise SystemExit(completed.stderr.strip() or completed.stdout.strip() or "closeout stage failed")
    closeout_json = package_dir / "closeout.json"
    if closeout_json.exists():
        stage["closeout_json"] = str(closeout_json)
        stage["closeout_sha256"] = sha256(closeout_json)
        closeout = json.loads(closeout_json.read_text())
        stage_history = closeout.get("stage_history_summary", {})
        if isinstance(stage_history, dict):
            stage["stage_history_sha256"] = stage_history.get("sha256")
            stage["stage_history_stage_count"] = stage_history.get("stage_count")
            stage["stage_history_remote_order_ids"] = stage_history.get("remote_order_ids")
    return stage


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text())
    if not isinstance(data, dict):
        raise SystemExit(f"JSON document must be an object: {path}")
    return data


def run(command: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, text=True, capture_output=True, check=False)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--release-zip", type=Path, default=DEFAULT_RELEASE_ZIP)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--external-references-file", type=Path, default=DEFAULT_EXTERNAL_REFERENCES)
    parser.add_argument("--candidate-market-file", type=Path)
    parser.add_argument("--market-url")
    parser.add_argument("--market-slug")
    parser.add_argument("--outcome")
    parser.add_argument("--human-review-ref")
    parser.add_argument("--exchange-rule-evidence-ref")
    parser.add_argument("--target-size")
    parser.add_argument("--max-order-notional-usd", default="1.00")
    parser.add_argument("--exchange-rule-valid-for-minutes", type=int, default=5)
    parser.add_argument(
        "--reviewed-go-decision-file",
        type=Path,
        help="Optional future reviewed-go decision input. The current pipeline records its presence but still does not run armed live phases.",
    )
    parser.add_argument(
        "--runtime-truth-file",
        type=Path,
        help="Optional durable runtime-truth evidence for future armed stage gating.",
    )
    parser.add_argument(
        "--closeout-package-dir",
        type=Path,
        help="Optional existing completed canary package directory for closeout/readback staging metadata.",
    )
    parser.add_argument("--root-ci-run-id", default="local-pipeline")
    parser.add_argument("--hermes-ci-run-id", default="local-pipeline")
    parser.add_argument("--execution-engine-ci-run-id", default="local-pipeline")
    parser.add_argument("--credentialed-sdk-run-id", required=True)
    return parser.parse_args()


def prepare_candidate(args: argparse.Namespace, output_dir: Path) -> tuple[Path | None, list[dict[str, Any]]]:
    stages: list[dict[str, Any]] = []
    if args.candidate_market_file:
        path = args.candidate_market_file
        path = path if path.is_absolute() else INTEGRATION_ROOT / path
        validate_candidate_file(
            path,
            max_order_notional_usd=parse_decimal(args.max_order_notional_usd, "max_order_notional_usd"),
            max_spread_bps=100,
        )
        stages.append(
            {
                "stage": "candidate_supplied",
                "status": "pass",
                "path": str(path),
                "exchange_rule_snapshot_bound": True,
            }
        )
        return path, stages

    if not (args.market_url or args.market_slug):
        stages.append(
            {
                "stage": "candidate_discovery",
                "status": "skipped",
                "reason": "no market-url or market-slug supplied; blocked rehearsal will use placeholder candidate",
            }
        )
        return None, stages

    if not args.outcome or not args.human_review_ref:
        raise SystemExit("--outcome, --human-review-ref, and --exchange-rule-evidence-ref are required when preparing a fresh candidate")

    candidate = output_dir / "candidate-market.json"
    audit = output_dir / "candidate-market.audit.json"
    command = [
        sys.executable,
        str(PREPARE_CANDIDATE),
        "--output",
        str(candidate),
        "--audit-output",
        str(audit),
        "--outcome",
        args.outcome,
        "--human-review-ref",
        args.human_review_ref,
        "--exchange-rule-evidence-ref",
        args.exchange_rule_evidence_ref,
        "--max-order-notional-usd",
        args.max_order_notional_usd,
        "--exchange-rule-valid-for-minutes",
        str(args.exchange_rule_valid_for_minutes),
    ]
    if args.market_url:
        command.extend(["--market-url", args.market_url])
    if args.market_slug:
        command.extend(["--market-slug", args.market_slug])
    if args.target_size:
        command.extend(["--target-size", args.target_size])
    completed = run(command, cwd=INTEGRATION_ROOT)
    (output_dir / "candidate-prep.stdout").write_text(completed.stdout)
    (output_dir / "candidate-prep.stderr").write_text(completed.stderr)
    if completed.returncode != 0:
        raise SystemExit(f"candidate preparation failed: {completed.stderr.strip() or completed.stdout.strip()}")
    stages.append(
        {
            "stage": "candidate_discovery",
            "status": "pass",
            "candidate_market_sha256": sha256(candidate),
            "path": str(candidate),
            "audit_path": str(audit),
        }
    )
    return candidate, stages


def main_with_args(args: argparse.Namespace) -> int:
    output_dir = args.output_dir if args.output_dir.is_absolute() else INTEGRATION_ROOT / args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    closeout_package_dir = (
        args.closeout_package_dir
        if args.closeout_package_dir is None or args.closeout_package_dir.is_absolute()
        else INTEGRATION_ROOT / args.closeout_package_dir
    )
    reviewed_go_decision = None
    if args.reviewed_go_decision_file:
        reviewed_go_decision_path = (
            args.reviewed_go_decision_file
            if args.reviewed_go_decision_file.is_absolute()
            else INTEGRATION_ROOT / args.reviewed_go_decision_file
        )
        reviewed_go_decision = validate_reviewed_go_decision_file(reviewed_go_decision_path)
    runtime_truth = None
    if args.runtime_truth_file:
        runtime_truth_path = (
            args.runtime_truth_file
            if args.runtime_truth_file.is_absolute()
            else INTEGRATION_ROOT / args.runtime_truth_file
        )
    external_references_file = (
        args.external_references_file
        if args.external_references_file.is_absolute()
        else INTEGRATION_ROOT / args.external_references_file
    )
    release_zip = args.release_zip if args.release_zip.is_absolute() else INTEGRATION_ROOT / args.release_zip
    manifest = args.manifest if args.manifest.is_absolute() else INTEGRATION_ROOT / args.manifest
    if not release_zip.exists():
        raise SystemExit(f"release zip missing: {release_zip}")
    if not manifest.exists():
        raise SystemExit(f"manifest missing: {manifest}")

    sidecar = release_zip.with_suffix(release_zip.suffix + ".evidence.json")
    sidecar_data = load_json(sidecar) if sidecar.exists() else {}
    canonical = sidecar_data.get("canonical_evidence", {}) if isinstance(sidecar_data, dict) else {}
    artifact_sha = sha256(release_zip)
    workspace_manifest_sha = canonical.get("workspace_manifest_sha256") or sha256(manifest)
    archived_manifest_sha = canonical.get("archived_manifest_sha256")
    if not archived_manifest_sha:
        raise SystemExit("release evidence sidecar missing archived manifest SHA-256")
    if args.runtime_truth_file:
        runtime_truth_path = (
            args.runtime_truth_file
            if args.runtime_truth_file.is_absolute()
            else INTEGRATION_ROOT / args.runtime_truth_file
        )
        runtime_truth = validate_runtime_truth_file(
            runtime_truth_path,
            expected_artifact_sha256=artifact_sha,
            expected_workspace_manifest_sha256=workspace_manifest_sha,
            expected_archived_manifest_sha256=archived_manifest_sha,
        )
    reviewed_go_present = reviewed_go_decision is not None
    runtime_truth_ready = bool(runtime_truth and runtime_truth.get("ready_for_armed_stage"))

    candidate, stages = prepare_candidate(args, output_dir)

    rehearsal_dir = output_dir / "no-go-blocked-rehearsal"
    command = [
        sys.executable,
        str(BLOCKED_REHEARSAL),
        "--output-dir",
        str(rehearsal_dir),
        "--external-references-file",
        str(external_references_file),
        "--artifact-sha256",
        artifact_sha,
        "--evidence-manifest-sha256",
        archived_manifest_sha,
        "--workspace-evidence-manifest-sha256",
        workspace_manifest_sha,
        "--archived-evidence-manifest-sha256",
        archived_manifest_sha,
        "--root-ci-run-id",
        args.root_ci_run_id,
        "--hermes-ci-run-id",
        args.hermes_ci_run_id,
        "--execution-engine-ci-run-id",
        args.execution_engine_ci_run_id,
        "--credentialed-sdk-run-id",
        args.credentialed_sdk_run_id,
    ]
    if candidate:
        command.extend(["--candidate-market-file", str(candidate)])
    completed = run(command, cwd=ENGINE)
    (output_dir / "blocked-rehearsal.stdout").write_text(completed.stdout)
    (output_dir / "blocked-rehearsal.stderr").write_text(completed.stderr)
    stages.append(
        {
            "stage": "no_go_blocked_rehearsal",
            "status": "pass" if completed.returncode == 0 else "fail",
            "exit_code": completed.returncode,
            "output_dir": str(rehearsal_dir),
        }
    )
    failures = []
    if completed.returncode != 0:
        failures.append("no-go blocked rehearsal failed")
    if closeout_package_dir:
        try:
            stages.append(run_closeout_stage(closeout_package_dir, release_zip))
        except SystemExit as exc:
            stages.append(
                {
                    "stage": "closeout",
                    "status": "fail",
                    "package_dir": str(closeout_package_dir),
                    "remote_side_effects": False,
                    "error": str(exc),
                }
            )
            failures.append("closeout stage failed")

    report = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "status": "pass" if not failures else "fail",
        "pipeline": "controlled_canary_fail_closed_preparation",
        "artifact_sha256": artifact_sha,
        "workspace_manifest_sha256": workspace_manifest_sha,
        "archived_manifest_sha256": archived_manifest_sha,
        "candidate_market_sha256": sha256(candidate) if candidate else None,
        "live_submit_authorized": False,
        "live_cancel_authorized": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "reviewed_go_created": False,
        "armed_cli_rehearsal_invoked": True,
        "armed_live_attempted": False,
        "stage_plan": build_stage_plan(
            reviewed_go=reviewed_go_present,
            closeout_package_dir=closeout_package_dir,
            runtime_truth_ready=runtime_truth_ready,
        ),
        "operator_runbook": build_operator_runbook(
            reviewed_go=reviewed_go_present,
            runtime_truth_ready=runtime_truth_ready,
            reviewed_go_decision=reviewed_go_decision,
        ),
        "runtime_truth_dependencies": runtime_truth_dependencies(),
        "runtime_truth": runtime_truth,
        "reviewed_go_decision": reviewed_go_decision,
        "stages": stages,
        "failures": failures,
    }
    (output_dir / "pipeline-report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if failures else 0


def main() -> int:
    args = parse_args()
    return main_with_args(args)


if __name__ == "__main__":
    raise SystemExit(main())
