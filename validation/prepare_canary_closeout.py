#!/usr/bin/env python3
"""Build a closeout report for a completed controlled canary package.

The script is intentionally read-only. It summarizes local package evidence and
release sidecars, and it refuses to turn partial readback into a stronger claim
than the evidence supports.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
VERSION = (INTEGRATION_ROOT / "VERSION").read_text().strip()
DEFAULT_RELEASE_ZIP = INTEGRATION_ROOT / "dist" / f"polymarket-execution-suite-v{VERSION}.zip"


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(INTEGRATION_ROOT))
    except ValueError:
        return str(path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare machine-readable and human closeout reports for a controlled canary package."
    )
    parser.add_argument(
        "--package-dir",
        type=Path,
        required=True,
        help="Exact canary review package directory to close out. Required to avoid selecting the wrong local order evidence.",
    )
    parser.add_argument("--release-zip", type=Path, default=DEFAULT_RELEASE_ZIP)
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--output-md", type=Path)
    return parser.parse_args()


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def required_json(package_dir: Path, name: str) -> dict[str, Any]:
    path = package_dir / name
    if not path.exists():
        raise SystemExit(f"required closeout evidence missing: {path}")
    return load_json(path)


def optional_json(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    return load_json(path)


def optional_package_json(package_dir: Path, name: str) -> dict[str, Any] | None:
    path = package_dir / name
    if not path.exists():
        return None
    return load_json(path)


def load_stage_history(package_dir: Path) -> tuple[Path, list[dict[str, Any]]]:
    path = package_dir / "post-canary-report.json.stages.jsonl"
    if not path.exists():
        raise SystemExit(f"required closeout stage history missing: {path}")
    stages: list[dict[str, Any]] = []
    for lineno, raw in enumerate(path.read_text().splitlines(), start=1):
        if not raw.strip():
            continue
        try:
            item = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"invalid stage history JSON on line {lineno}: {exc}") from exc
        if not isinstance(item, dict):
            raise SystemExit(f"invalid stage history entry on line {lineno}: expected object")
        stages.append(item)
    if not stages:
        raise SystemExit(f"required closeout stage history is empty: {path}")
    return path, stages


def decimal_text(value: Any) -> str:
    try:
        parsed = Decimal(str(value))
    except (InvalidOperation, ValueError):
        raise SystemExit(f"invalid decimal value in closeout evidence: {value!r}")
    if not parsed.is_finite():
        raise SystemExit(f"non-finite decimal value in closeout evidence: {value!r}")
    normalized = parsed.normalize()
    text = format(normalized, "f")
    return "0" if text == "-0" else text


def decimal_value(value: Any) -> Decimal:
    try:
        parsed = Decimal(str(value))
    except (InvalidOperation, ValueError):
        raise SystemExit(f"invalid decimal value in closeout evidence: {value!r}")
    if not parsed.is_finite():
        raise SystemExit(f"non-finite decimal value in closeout evidence: {value!r}")
    return parsed


def lget(data: dict[str, Any], *path: str, default: Any = None) -> Any:
    current: Any = data
    for part in path:
        if not isinstance(current, dict) or part not in current:
            return default
        current = current[part]
    return current


def summarize_stage_history(
    stage_history_path: Path,
    stages: list[dict[str, Any]],
    expected_order_id: Any,
) -> dict[str, Any]:
    names = [str(stage.get("stage", "")) for stage in stages]
    statuses = [str(stage.get("status", "")) for stage in stages]
    remote_order_ids = sorted(
        {
            str(stage["remote_order_id"])
            for stage in stages
            if isinstance(stage.get("remote_order_id"), str) and stage["remote_order_id"]
        }
    )
    operator_required = [
        {
            "stage": stage.get("stage"),
            "status": stage.get("status"),
            "remote_order_id": stage.get("remote_order_id"),
            "error_summary": stage.get("error_summary"),
        }
        for stage in stages
        if stage.get("operator_required") is True or stage.get("status") == "operator_required"
    ]
    raw_signed_order_exposed = any(stage.get("raw_signed_order_exposed") is True for stage in stages)
    has_post_accepted = any(
        stage.get("stage") == "post_accepted"
        and stage.get("posted") is True
        and bool(stage.get("remote_order_id"))
        for stage in stages
    )
    has_approval_consumed_post_remote_attempt = any(
        stage.get("stage") == "approval_consumed_post_remote_attempt"
        and bool(stage.get("remote_order_id"))
        for stage in stages
    )
    has_cancel_confirmed = any(
        stage.get("stage") == "cancel_confirmed"
        and stage.get("posted") is True
        and stage.get("cancelled") is True
        and bool(stage.get("remote_order_id"))
        for stage in stages
    )
    expected_order_text = str(expected_order_id) if expected_order_id else ""
    order_matches = bool(expected_order_text) and remote_order_ids == [expected_order_text]
    return {
        "path": display_path(stage_history_path),
        "sha256": sha256(stage_history_path),
        "stage_count": len(stages),
        "stages": names,
        "statuses": statuses,
        "remote_order_ids": remote_order_ids,
        "operator_required_stages": operator_required,
        "has_post_accepted": has_post_accepted,
        "has_approval_consumed_post_remote_attempt": has_approval_consumed_post_remote_attempt,
        "has_cancel_confirmed": has_cancel_confirmed,
        "raw_signed_order_exposed": raw_signed_order_exposed,
        "remote_order_matches_report": order_matches,
    }


def validate_operator_recovery(
    package_dir: Path,
    stage_history_summary: dict[str, Any],
    expected_order_id: Any,
    candidate_sha256: str,
) -> dict[str, Any]:
    operator_required = stage_history_summary["operator_required_stages"]
    path = package_dir / "operator-recovery.json"
    incident_path = package_dir / "operator-incident-recovery.json"
    if not operator_required:
        if path.exists():
            raise SystemExit(f"operator recovery evidence is present but no operator_required stage exists: {path}")
        if incident_path.exists():
            raise SystemExit(f"incident recovery evidence is present but no operator_required stage exists: {incident_path}")
        return {
            "status": "not_required",
            "path": None,
            "sha256": None,
            "operator_review_ref": None,
            "stage_history_sha256": stage_history_summary["sha256"],
        }
    has_post_unknown_without_order = any(
        item.get("stage") == "post_unknown" and not item.get("remote_order_id")
        for item in operator_required
    )
    if has_post_unknown_without_order:
        if path.exists():
            raise SystemExit("operator recovery evidence cannot close post_unknown without remote_order_id; use operator-incident-recovery.json")
        return validate_incident_recovery(
            package_dir,
            stage_history_summary,
            candidate_sha256,
        )
    if not path.exists():
        raise SystemExit(f"operator_required stage requires operator recovery evidence: {path}")
    if incident_path.exists():
        raise SystemExit("incident recovery evidence is only valid for post_unknown without remote_order_id")
    recovery = load_json(path)
    required_readback = {
        "order-status-query.json",
        "trade-fill-query.json",
        "account-activity-readback.json",
    }
    readback_evidence = recovery.get("readback_evidence", [])
    if not isinstance(readback_evidence, list):
        raise SystemExit("operator recovery evidence invalid: readback_evidence must be a list")
    checks = {
        "schema_version": recovery.get("schema_version") == 1,
        "recovery_decision": recovery.get("recovery_decision") == "operator_reviewed_closed_no_retry",
        "operator_review_ref": isinstance(recovery.get("operator_review_ref"), str)
        and bool(recovery["operator_review_ref"].strip()),
        "stage_history_sha256": recovery.get("stage_history_sha256")
        == stage_history_summary["sha256"],
        "remote_order_id": str(recovery.get("remote_order_id", ""))
        == str(expected_order_id),
        "unresolved_operator_required": recovery.get("unresolved_operator_required") is False,
        "no_retry_authorized": recovery.get("no_retry_authorized") is True,
        "no_second_order_placed": recovery.get("no_second_order_placed") is True,
        "raw_signed_order_exposed": recovery.get("raw_signed_order_exposed") is False,
        "readback_evidence": required_readback.issubset({str(item) for item in readback_evidence}),
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise SystemExit("operator recovery evidence checks failed: " + ", ".join(failed))
    return {
        "status": "recovered",
        "path": display_path(path),
        "sha256": sha256(path),
        "operator_review_ref": recovery["operator_review_ref"],
        "stage_history_sha256": recovery["stage_history_sha256"],
        "remote_order_id": recovery["remote_order_id"],
        "recovery_decision": recovery["recovery_decision"],
        "operator_required_stage_count": len(operator_required),
        "readback_evidence": readback_evidence,
    }


def validate_incident_recovery(
    package_dir: Path,
    stage_history_summary: dict[str, Any],
    candidate_sha256: str,
) -> dict[str, Any]:
    path = package_dir / "operator-incident-recovery.json"
    if not path.exists():
        raise SystemExit(f"post_unknown without remote_order_id requires incident recovery evidence: {path}")
    recovery = load_json(path)
    open_orders = required_json(package_dir, "account-open-orders-readback.json")
    trade_history = required_json(package_dir, "account-trade-history-readback.json")
    account_activity = required_json(package_dir, "account-activity-readback.json")
    account_level_evidence = recovery.get("account_level_evidence", [])
    if not isinstance(account_level_evidence, list):
        raise SystemExit("incident recovery evidence invalid: account_level_evidence must be a list")
    required_evidence = {
        "account-open-orders-readback.json",
        "account-trade-history-readback.json",
        "account-activity-readback.json",
    }
    window = recovery.get("investigation_window", {})
    checks = {
        "schema_version": recovery.get("schema_version") == 1,
        "recovery_decision": recovery.get("recovery_decision")
        == "operator_reviewed_no_remote_order_found_no_retry",
        "operator_review_ref": isinstance(recovery.get("operator_review_ref"), str)
        and bool(recovery["operator_review_ref"].strip()),
        "stage_history_sha256": recovery.get("stage_history_sha256")
        == stage_history_summary["sha256"],
        "candidate_market_sha256": recovery.get("candidate_market_sha256") == candidate_sha256,
        "remote_order_id": recovery.get("remote_order_id") is None,
        "investigation_window_started_at": isinstance(window, dict)
        and isinstance(window.get("started_at"), str)
        and bool(window["started_at"].strip()),
        "investigation_window_ended_at": isinstance(window, dict)
        and isinstance(window.get("ended_at"), str)
        and bool(window["ended_at"].strip()),
        "unresolved_operator_required": recovery.get("unresolved_operator_required") is False,
        "no_retry_authorized": recovery.get("no_retry_authorized") is True,
        "no_second_order_placed": recovery.get("no_second_order_placed") is True,
        "raw_signed_order_exposed": recovery.get("raw_signed_order_exposed") is False,
        "account_level_evidence": required_evidence.issubset(
            {str(item) for item in account_level_evidence}
        ),
        "incident_recovery_no_matching_open_orders": lget(
            open_orders, "matching_open_orders_count", default=0
        )
        == 0,
        "incident_recovery_no_matching_trades": lget(
            trade_history, "matching_trades_count", default=0
        )
        == 0,
        "incident_recovery_zero_matching_trade_size": decimal_text(
            lget(trade_history, "matching_size_total", default="0")
        )
        == "0",
        "incident_recovery_no_matching_activity": lget(
            account_activity, "matching_activity_count", default=0
        )
        == 0,
        "incident_recovery_no_raw_signed_order_exposed": not any(
            bool(lget(item, "raw_signed_order_exposed", default=False))
            for item in [open_orders, trade_history, account_activity]
        ),
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise SystemExit("incident recovery evidence checks failed: " + ", ".join(failed))
    return {
        "status": "incident_recovered_no_remote_order_found",
        "path": display_path(path),
        "sha256": sha256(path),
        "operator_review_ref": recovery["operator_review_ref"],
        "stage_history_sha256": recovery["stage_history_sha256"],
        "candidate_market_sha256": recovery["candidate_market_sha256"],
        "recovery_decision": recovery["recovery_decision"],
        "operator_required_stage_count": len(stage_history_summary["operator_required_stages"]),
        "investigation_window": window,
        "account_level_evidence": account_level_evidence,
        "incident_checks": checks,
    }


def build_closeout(package_dir: Path, release_zip: Path) -> dict[str, Any]:
    report = required_json(package_dir, "post-canary-report.json")
    candidate = required_json(package_dir, "candidate-market.json")
    order_status = optional_package_json(package_dir, "order-status-query.json")
    trade_query = optional_package_json(package_dir, "trade-fill-query.json")
    account_activity = required_json(package_dir, "account-activity-readback.json")
    stage_history_path, stage_history = load_stage_history(package_dir)
    sidecar = optional_json(release_zip.with_suffix(release_zip.suffix + ".evidence.json"))
    candidate_sha256 = sha256(package_dir / "candidate-market.json")

    limit_price_value = decimal_value(candidate["limit_price"])
    target_size_value = decimal_value(candidate["target_size"])
    notional_value = limit_price_value * target_size_value
    limit_price = decimal_text(limit_price_value)
    target_size = decimal_text(target_size_value)
    notional = decimal_text(notional_value)
    remote_order_id = lget(report, "remote_order_readback", "order_id")
    if remote_order_id is None:
        remote_order_id = report.get("remote_order_id")
    stage_history_summary = summarize_stage_history(
        stage_history_path,
        stage_history,
        remote_order_id,
    )
    operator_recovery_summary = validate_operator_recovery(
        package_dir,
        stage_history_summary,
        remote_order_id,
        candidate_sha256,
    )
    incident_closeout = operator_recovery_summary["status"] == "incident_recovered_no_remote_order_found"

    checks = {
        "candidate_side_buy": candidate.get("side") == "BUY",
        "candidate_order_type_gtc": candidate.get("order_type") == "GTC",
        "candidate_post_only": candidate.get("post_only") is True,
        "candidate_uses_target_size_as_size": (
            decimal_value(lget(report, "market_candidate", "target_size"))
            if lget(report, "market_candidate", "target_size") is not None
            else target_size_value
        )
        == target_size_value,
        "notional_is_price_times_size": (
            decimal_value(lget(report, "market_candidate", "notional_usd"))
            if lget(report, "market_candidate", "notional_usd") is not None
            else notional_value
        )
        == notional_value,
        "order_remote_status_canceled": incident_closeout
        or lget(order_status or {}, "remote_status") == "CANCELED",
        "order_size_matched_zero": incident_closeout
        or decimal_text(lget(order_status or {}, "size_matched", default="0")) == "0",
        "trade_query_zero_matching_trades": incident_closeout
        or lget(trade_query or {}, "matching_trades_count") == 0,
        "trade_query_zero_matching_size": incident_closeout
        or decimal_text(lget(trade_query or {}, "matching_size_total", default="0")) == "0",
        "account_activity_zero_matching_activity": lget(account_activity, "matching_activity_count") == 0,
        "account_activity_zero_matching_trades": lget(account_activity, "matching_trade_count") == 0,
        "account_activity_zero_open_positions": lget(account_activity, "matching_open_position_count") == 0,
        "account_activity_zero_closed_positions": lget(account_activity, "matching_closed_position_count") == 0,
        "account_activity_value_records_consistent": (
            lget(account_activity, "matching_value_record_count", default=0)
            == len([item for item in account_activity.get("values", []) if isinstance(item, dict)])
        ),
        "account_activity_value_zero": all(
            decimal_text(item.get("value", "0")) == "0"
            for item in account_activity.get("values", [])
            if isinstance(item, dict)
        ),
        "no_raw_signed_order_exposed": not any(
            bool(lget(item, "raw_signed_order_exposed", default=False))
            for item in [report, order_status, trade_query, account_activity]
        ),
        "stage_history_has_post_accepted": incident_closeout
        or stage_history_summary["has_post_accepted"],
        "stage_history_has_approval_consumed_post_remote_attempt": incident_closeout
        or stage_history_summary["has_approval_consumed_post_remote_attempt"],
        "stage_history_has_cancel_confirmed": incident_closeout
        or stage_history_summary["has_cancel_confirmed"]
        or operator_recovery_summary["status"] == "recovered",
        "stage_history_post_unknown_incident_recovered": (
            not incident_closeout
            or (
                "post_unknown" in stage_history_summary["stages"]
                and not stage_history_summary["remote_order_ids"]
            )
        ),
        "stage_history_operator_required_recovered": (
            stage_history_summary["operator_required_stages"] == []
            or operator_recovery_summary["status"] in {
                "recovered",
                "incident_recovered_no_remote_order_found",
            }
        ),
        "stage_history_no_raw_signed_order_exposed": not stage_history_summary[
            "raw_signed_order_exposed"
        ],
        "stage_history_remote_order_matches_report": stage_history_summary[
            "remote_order_matches_report"
        ]
        or incident_closeout,
        "incident_recovery_no_matching_open_orders": (
            not incident_closeout
            or operator_recovery_summary["incident_checks"][
                "incident_recovery_no_matching_open_orders"
            ]
        ),
        "incident_recovery_no_matching_trades": (
            not incident_closeout
            or operator_recovery_summary["incident_checks"][
                "incident_recovery_no_matching_trades"
            ]
        ),
        "incident_recovery_no_matching_activity": (
            not incident_closeout
            or operator_recovery_summary["incident_checks"][
                "incident_recovery_no_matching_activity"
            ]
        ),
        "no_second_order_placed_by_closeout": (
            report.get("no_second_order_placed_by_closure") is True
            or (
                stage_history_summary["operator_required_stages"] == []
                and len(stage_history_summary["remote_order_ids"]) <= 1
            )
        ),
        "release_sidecar_matches_zip_hash": (
            not isinstance(sidecar, dict)
            or lget(sidecar, "artifact", "sha256") is None
            or lget(sidecar, "artifact", "sha256") == sha256(release_zip)
        ),
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise SystemExit("closeout evidence checks failed: " + ", ".join(failed))

    evidence_files = []
    for path in sorted(package_dir.iterdir()):
        if path.is_file():
            evidence_files.append(
                {
                    "path": path.name,
                    "sha256": sha256(path),
                    "bytes": path.stat().st_size,
                }
            )

    release_binding: dict[str, Any] = {
        "release_zip_path": display_path(release_zip) if release_zip.exists() else str(release_zip),
        "release_zip_sha256": sha256(release_zip) if release_zip.exists() else None,
        "release_evidence_sidecar_path": display_path(release_zip.with_suffix(release_zip.suffix + ".evidence.json"))
        if release_zip.with_suffix(release_zip.suffix + ".evidence.json").exists()
        else None,
    }
    if isinstance(sidecar, dict):
        release_binding.update(
            {
                "sidecar_artifact_sha256": lget(sidecar, "artifact", "sha256"),
                "sidecar_git_head": lget(sidecar, "source", "git_head"),
                "sidecar_submodules": lget(sidecar, "source", "submodules", default=[]),
                "canonical_evidence_manifest_sha256": lget(
                    sidecar, "canonical_evidence", "archived_manifest_sha256"
                ),
            }
        )

    return {
        "schema_version": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "package": display_path(package_dir),
        "decision": "controlled_real_funds_canary_incident_closed_no_remote_order_found"
        if incident_closeout
        else "controlled_real_funds_canary_closed",
        "release_binding": release_binding,
        "remote_order_id": remote_order_id,
        "stage_history_summary": stage_history_summary,
        "operator_recovery_summary": operator_recovery_summary,
        "canary_semantics": {
            "scope": "REAL_FUNDS_CANARY",
            "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
            "side": "BUY",
            "order_type": "GTC",
            "post_only": True,
            "target_size_semantics": "outcome_shares",
            "size": target_size,
            "limit_price": limit_price,
            "notional_usd": notional,
            "notional_rule": "limit_price * size",
            "min_funds_goal": "minimize expected funds at risk while satisfying current exchange size rule",
        },
        "readback_summary": {
            "remote_status": None if incident_closeout else lget(order_status or {}, "remote_status"),
            "size_matched": None if incident_closeout else lget(order_status or {}, "size_matched"),
            "matching_trades_count": lget(
                trade_query or {}, "matching_trades_count", default=0
            ),
            "matching_size_total": lget(
                trade_query or {}, "matching_size_total", default="0"
            ),
            "matching_activity_count": lget(account_activity, "matching_activity_count"),
            "matching_trade_count": lget(account_activity, "matching_trade_count"),
            "matching_open_position_count": lget(account_activity, "matching_open_position_count"),
            "matching_closed_position_count": lget(account_activity, "matching_closed_position_count"),
            "matching_value_record_count": lget(account_activity, "matching_value_record_count"),
            "value_records": account_activity.get("values", []),
            "raw_signed_order_exposed": False,
        },
        "evidence_checks": checks,
        "evidence_files": evidence_files,
        "limitations": [
            "This is a closeout over order status, trade query, and public Data API account activity endpoints.",
            "It is not a formal exchange/account statement export.",
            "It does not authorize a second real-funds canary or production/live trading.",
        ],
        "next_required_actions": [
            "Use a fresh reviewed release decision before any future armed canary.",
            "Regenerate candidate-market.json when exchange min size, order type support, or tick rules change.",
            "Keep GTC post-only cancel semantics separate from general live submit semantics.",
        ],
    }


def markdown(closeout: dict[str, Any]) -> str:
    checks = closeout["evidence_checks"]
    release = closeout["release_binding"]
    semantics = closeout["canary_semantics"]
    readback = closeout["readback_summary"]
    lines = [
        "# Controlled Canary Closeout",
        "",
        f"- Package: `{closeout['package']}`",
        f"- Decision: `{closeout['decision']}`",
        f"- Release zip SHA-256: `{release.get('release_zip_sha256')}`",
        f"- Root git head: `{release.get('sidecar_git_head')}`",
        f"- Remote order id: `{closeout['remote_order_id']}`",
        f"- Stage history SHA-256: `{closeout['stage_history_summary']['sha256']}`",
        f"- Operator recovery: `{closeout['operator_recovery_summary']['status']}`",
        "",
        "## Semantics",
        "",
        f"- Execution style: `{semantics['execution_style']}`",
        f"- Order: `{semantics['side']}/{semantics['order_type']}` post-only",
        f"- Size: `{semantics['size']}` outcome shares",
        f"- Limit price: `{semantics['limit_price']}`",
        f"- Notional rule: `{semantics['notional_rule']}` = `{semantics['notional_usd']}`",
        "",
        "## Readback",
        "",
        f"- Remote status: `{readback['remote_status']}`",
        f"- Size matched: `{readback['size_matched']}`",
        f"- Matching trades: `{readback['matching_trades_count']}`",
        f"- Matching account activity: `{readback['matching_activity_count']}`",
        f"- Matching open positions: `{readback['matching_open_position_count']}`",
        f"- Matching closed positions: `{readback['matching_closed_position_count']}`",
        f"- Stage history entries: `{closeout['stage_history_summary']['stage_count']}`",
        f"- Operator-required stages: `{len(closeout['stage_history_summary']['operator_required_stages'])}`",
        "",
        "## Checks",
        "",
    ]
    for name, ok in checks.items():
        lines.append(f"- `{name}`: `{str(ok).lower()}`")
    lines.extend(
        [
            "",
            "## Limitations",
            "",
            "- This is not a formal exchange/account statement export.",
            "- This does not authorize another canary or production/live trading.",
            "",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    package_dir = args.package_dir.resolve()
    release_zip = args.release_zip.resolve()
    closeout = build_closeout(package_dir, release_zip)
    output_json = args.output_json or package_dir / "closeout.json"
    output_md = args.output_md or package_dir / "CLOSEOUT.md"
    output_json.write_text(json.dumps(closeout, indent=2, sort_keys=True) + "\n")
    output_md.write_text(markdown(closeout))
    print(json.dumps({"status": "ok", "json": str(output_json), "md": str(output_md)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
