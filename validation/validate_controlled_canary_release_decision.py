#!/usr/bin/env python3
"""Validate controlled-canary release decision templates remain fail-closed."""
from __future__ import annotations

import json
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config"
TEMPLATE = CONFIG / "controlled-canary.release-decision.template.json"
EXAMPLE = CONFIG / "controlled-canary.release-decision.example.json"
INVALID_PARTIAL = CONFIG / "controlled-canary.release-decision.invalid-partial.fixture.json"
INVALID_MISMATCHED = CONFIG / "controlled-canary.release-decision.invalid-mismatched.fixture.json"
INVALID_STATUS = CONFIG / "controlled-canary.release-decision.invalid-status.fixture.json"

EXPECTED_ARTIFACT_SHA256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256 = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
EXPECTED_REVIEWED_EXAMPLE_WORKSPACE_MANIFEST_SHA256 = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
EXPECTED_MARKET_CANDIDATE_SHA256 = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
EXPECTED_RUN_IDS = {
    "root_ci_run_id": "26268697168",
    "hermes_ci_run_id": "26267887116",
    "execution_engine_ci_run_id": "26268276210",
    "credentialed_sdk_run_id": "local-current-gates-20260523",
}
AUTHORIZATION_FLAGS = [
    "live_submit_authorized",
    "live_cancel_authorized",
    "production_deployment_authorized",
    "real_funds_canary_authorized",
    "remote_side_effects_authorized",
]
ALLOWED_TOP_LEVEL_FIELDS = {
    "schema_version",
    "decision_id",
    "status",
    "source_release",
    "decision",
    "decision_reason",
    "scope",
    "execution_style",
    "expires_at",
    "artifact_sha256",
    "evidence_manifest_sha256",
    "workspace_manifest_sha256",
    "archived_manifest_sha256",
    "market_candidate_sha256",
    "github_evidence",
    "external_references",
    "risk_limits",
    "required_review_signals",
    "live_submit_authorized",
    "live_cancel_authorized",
    "production_deployment_authorized",
    "real_funds_canary_authorized",
    "remote_side_effects_authorized",
    "single_attempt",
    "max_order_count",
    "post_cancel_required",
    "readback_closeout_required",
    "allow_real_funds_canary",
    "reviewed_release_decision_present",
    "operator_identity_ref",
    "secrets_included",
}
REQUIRED_EXTERNAL_REFS = [
    "secret_custody_ref",
    "operator_approval_ref",
    "alert_routing_ref",
    "dashboard_ref",
    "rollback_runbook_ref",
    "incident_runbook_ref",
]
REQUIRED_REVIEW_SIGNALS = [
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
FORBIDDEN_TEXT_TOKENS = [
    "private_key",
    "clob_secret",
    "api_secret",
    "raw_signature",
    "raw_signed_payload",
    "signed_order_envelope",
]


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def expected_source_release() -> str:
    in_workspace_package = False
    for line in (ROOT / "Cargo.toml").read_text().splitlines():
        stripped = line.strip()
        if stripped == "[workspace.package]":
            in_workspace_package = True
            continue
        if stripped.startswith("[") and in_workspace_package:
            break
        if in_workspace_package and stripped.startswith("version = "):
            return "v" + stripped.split("=", 1)[1].strip().strip('"')
    raise SystemExit("could not read workspace package version from Cargo.toml")


def is_sha256(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(ch in "0123456789abcdefABCDEF" for ch in value)


def parse_time(value: object) -> datetime | None:
    if not isinstance(value, str) or value.startswith("REPLACE_WITH_"):
        return None
    try:
        normalized = value.replace("Z", "+00:00")
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed


def parse_positive_decimal(value: object) -> Decimal | None:
    if not isinstance(value, str) or value.startswith("REPLACE_WITH_"):
        return None
    try:
        parsed = Decimal(value)
    except (InvalidOperation, ValueError):
        return None
    if not parsed.is_finite() or parsed <= 0:
        return None
    return parsed


def has_placeholder(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith("REPLACE_WITH_")
    if isinstance(value, dict):
        return any(has_placeholder(child) for child in value.values())
    if isinstance(value, list):
        return any(has_placeholder(child) for child in value)
    return False


def validate_shape(data: dict[str, Any], label: str) -> list[str]:
    failures: list[str] = []
    unknown_fields = sorted(set(data) - ALLOWED_TOP_LEVEL_FIELDS)
    if unknown_fields:
        failures.append(f"{label}: unknown fields not accepted by Rust model: {', '.join(unknown_fields)}")
    if data.get("schema_version") != 1:
        failures.append(f"{label}: schema_version must be 1")
    source_release = expected_source_release()
    if data.get("source_release") != source_release:
        failures.append(f"{label}: source_release must bind {source_release}")
    if data.get("scope") != "REAL_FUNDS_CANARY":
        failures.append(f"{label}: scope must be REAL_FUNDS_CANARY")
    if data.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
        failures.append(f"{label}: execution_style must be GTC_LIMIT_POST_ONLY_CANCEL")
    limits = data.get("risk_limits", {})
    max_order_notional = parse_positive_decimal(limits.get("max_order_notional_usd"))
    if max_order_notional is None or max_order_notional > Decimal("1"):
        failures.append(f"{label}: max_order_notional_usd must be positive and <= 1")
    max_daily_notional = parse_positive_decimal(limits.get("max_daily_notional_usd"))
    if max_daily_notional is None or max_daily_notional > Decimal("5"):
        failures.append(f"{label}: max_daily_notional_usd must be positive and <= 5")
    if data.get("secrets_included") is not False:
        failures.append(f"{label}: secrets_included must be false")
    workspace_sha = data.get("workspace_manifest_sha256")
    archived_sha = data.get("archived_manifest_sha256")
    evidence_sha = data.get("evidence_manifest_sha256")
    if label == "template":
        if not has_placeholder(workspace_sha) or not has_placeholder(archived_sha):
            failures.append(f"{label}: workspace/archived manifest hashes must remain placeholders")
    else:
        if not is_sha256(workspace_sha):
            failures.append(f"{label}: workspace_manifest_sha256 must be 64-hex")
        if archived_sha != evidence_sha:
            failures.append(f"{label}: archived_manifest_sha256 must equal evidence_manifest_sha256")
    for flag in AUTHORIZATION_FLAGS:
        if flag not in data:
            failures.append(f"{label}: missing {flag}")
    if "allow_real_funds_canary" not in data:
        failures.append(f"{label}: missing allow_real_funds_canary")
    if "reviewed_release_decision_present" not in data:
        failures.append(f"{label}: missing reviewed_release_decision_present")
    if decision := data.get("decision"):
        if decision == "go":
            if data.get("single_attempt") is not True:
                failures.append(f"{label}: go decision must set single_attempt=true")
            if data.get("max_order_count") != 1:
                failures.append(f"{label}: go decision must set max_order_count=1")
            if data.get("post_cancel_required") is not True:
                failures.append(f"{label}: go decision must set post_cancel_required=true")
            if data.get("readback_closeout_required") is not True:
                failures.append(f"{label}: go decision must set readback_closeout_required=true")
    if not data.get("operator_identity_ref"):
        failures.append(f"{label}: operator_identity_ref must be concrete")
    elif label != "template" and has_placeholder(data.get("operator_identity_ref")):
        failures.append(f"{label}: operator_identity_ref must be concrete")
    refs = data.get("external_references")
    if not isinstance(refs, dict):
        failures.append(f"{label}: external_references must be an object")
    else:
        for key in REQUIRED_EXTERNAL_REFS:
            if key not in refs:
                failures.append(f"{label}: missing external reference {key}")
    signals = data.get("required_review_signals")
    if not isinstance(signals, dict):
        failures.append(f"{label}: required_review_signals must be an object")
    else:
        for key in REQUIRED_REVIEW_SIGNALS:
            if key not in signals:
                failures.append(f"{label}: missing review signal {key}")
            elif not isinstance(signals[key], bool):
                failures.append(f"{label}: review signal {key} must be boolean")
    lowered = json.dumps(data, sort_keys=True).lower()
    for token in FORBIDDEN_TEXT_TOKENS:
        if token in lowered:
            failures.append(f"{label}: forbidden sensitive token appears in decision document: {token}")
    return failures


def validate_decision(data: dict[str, Any], label: str) -> list[str]:
    failures = validate_shape(data, label)
    decision = data.get("decision")
    authorized = any(data.get(flag) is True for flag in AUTHORIZATION_FLAGS)
    refs = data.get("external_references", {})
    signals = data.get("required_review_signals", {})
    expires_at = parse_time(data.get("expires_at"))
    now = datetime.now(timezone.utc)

    if decision not in {"no_go", "go"}:
        failures.append(f"{label}: decision must be no_go or go")
    if decision == "no_go":
        if authorized or data.get("allow_real_funds_canary") is True:
            failures.append(f"{label}: no_go decision must not authorize live or remote side effects")
        return failures

    if decision == "go":
        if data.get("status") != "reviewed_go":
            failures.append(f"{label}: go decision requires status=reviewed_go")
        if not all(data.get(flag) is True for flag in ["live_submit_authorized", "live_cancel_authorized", "real_funds_canary_authorized", "remote_side_effects_authorized"]):
            failures.append(f"{label}: go decision must explicitly authorize the controlled canary side-effect flags")
        if data.get("production_deployment_authorized") is True:
            failures.append(f"{label}: go decision must not authorize production deployment")
        if not is_sha256(data.get("artifact_sha256")):
            failures.append(f"{label}: go decision requires concrete artifact_sha256")
        if not is_sha256(data.get("evidence_manifest_sha256")):
            failures.append(f"{label}: go decision requires concrete evidence_manifest_sha256")
        if not is_sha256(data.get("market_candidate_sha256")):
            failures.append(f"{label}: go decision requires concrete market_candidate_sha256")
        if data.get("allow_real_funds_canary") is not True:
            failures.append(f"{label}: go decision must set allow_real_funds_canary=true")
        if data.get("reviewed_release_decision_present") is not True:
            failures.append(f"{label}: go decision must set reviewed_release_decision_present=true")
        missing_refs = [key for key in REQUIRED_EXTERNAL_REFS if not refs.get(key) or has_placeholder(refs.get(key))]
        if missing_refs:
            failures.append(f"{label}: go decision missing external references: {', '.join(missing_refs)}")
        missing_signals = [key for key in REQUIRED_REVIEW_SIGNALS if signals.get(key) is not True]
        if missing_signals:
            failures.append(f"{label}: go decision missing review signals: {', '.join(missing_signals)}")
        if expires_at is None:
            failures.append(f"{label}: go decision requires parseable expires_at")
        elif expires_at <= now:
            failures.append(f"{label}: go decision is expired")
    return failures


def main() -> int:
    failures: list[str] = []
    for path in [TEMPLATE, EXAMPLE, INVALID_PARTIAL, INVALID_MISMATCHED, INVALID_STATUS]:
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")
    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    template = load(TEMPLATE)
    example = load(EXAMPLE)
    invalid = load(INVALID_PARTIAL)
    invalid_mismatched = load(INVALID_MISMATCHED)
    invalid_status = load(INVALID_STATUS)

    failures.extend(validate_decision(template, "template"))
    if template.get("decision") != "no_go" or template.get("status") != "template_not_reviewed":
        failures.append("template must default to template_not_reviewed no_go")
    if not has_placeholder(template.get("artifact_sha256")) or not has_placeholder(template.get("evidence_manifest_sha256")):
        failures.append("template must keep artifact/evidence hashes as placeholders")
    if not has_placeholder(template.get("market_candidate_sha256")):
        failures.append("template must keep market candidate hash as a placeholder")
    if any(template.get(flag) is not False for flag in AUTHORIZATION_FLAGS):
        failures.append("template must keep all authorization flags false")

    failures.extend(validate_decision(example, "example"))
    if example.get("artifact_sha256") != EXPECTED_ARTIFACT_SHA256:
        failures.append("example must bind the illustrative current example artifact hash")
    if example.get("evidence_manifest_sha256") != EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256:
        failures.append("example must bind the illustrative current example evidence manifest hash")
    if example.get("workspace_manifest_sha256") != EXPECTED_REVIEWED_EXAMPLE_WORKSPACE_MANIFEST_SHA256:
        failures.append("example must bind the illustrative current example workspace manifest hash")
    if example.get("archived_manifest_sha256") != EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256:
        failures.append("example must bind the illustrative current example archived manifest hash")
    if example.get("market_candidate_sha256") != EXPECTED_MARKET_CANDIDATE_SHA256:
        failures.append("example must bind the illustrative current example market candidate hash")
    for key, expected in EXPECTED_RUN_IDS.items():
        if example.get("github_evidence", {}).get(key) != expected:
            failures.append(f"example must bind GitHub evidence run {key}")

    invalid_failures = validate_decision(invalid, "invalid_partial")
    if not invalid_failures:
        failures.append("invalid partial fixture must be rejected")
    expected_rejection_tokens = ["missing external references", "missing review signals", "expired"]
    invalid_text = "\n".join(invalid_failures)
    for token in expected_rejection_tokens:
        if token not in invalid_text:
            failures.append(f"invalid partial fixture rejection missing token: {token}")

    invalid_status_failures = validate_decision(invalid_status, "invalid_status")
    if not invalid_status_failures:
        failures.append("invalid status fixture must be rejected")
    invalid_status_text = "\n".join(invalid_status_failures)
    if "status" not in invalid_status_text or "reviewed_go" not in invalid_status_text:
        failures.append("invalid status fixture rejection missing reviewed_go status token")

    mismatched_failures = validate_decision(invalid_mismatched, "invalid_mismatched")
    if invalid_mismatched.get("artifact_sha256") != EXPECTED_ARTIFACT_SHA256:
        mismatched_failures.append("invalid_mismatched: artifact hash does not match reviewed fixture")
    if invalid_mismatched.get("evidence_manifest_sha256") != EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256:
        mismatched_failures.append("invalid_mismatched: evidence manifest hash does not match reviewed fixture")
    if invalid_mismatched.get("market_candidate_sha256") != EXPECTED_MARKET_CANDIDATE_SHA256:
        mismatched_failures.append("invalid_mismatched: market candidate hash does not match reviewed fixture")
    if not mismatched_failures:
        failures.append("invalid mismatched fixture must be rejected")
    mismatched_text = "\n".join(mismatched_failures)
    for token in ["artifact hash does not match", "evidence manifest hash does not match", "market candidate hash does not match"]:
        if token not in mismatched_text:
            failures.append(f"invalid mismatched fixture rejection missing token: {token}")

    result = {
        "status": "fail" if failures else "pass",
        "template_default_decision": template.get("decision"),
        "example_decision": example.get("decision"),
        "invalid_partial_rejected": bool(invalid_failures),
        "invalid_status_rejected": bool(invalid_status_failures),
        "invalid_mismatched_rejected": bool(mismatched_failures),
        "live_submit_authorized": False,
        "live_cancel_authorized": False,
        "production_deployment_authorized": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects_authorized": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
