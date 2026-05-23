#!/usr/bin/env python3
"""Prepare a local real-funds canary review package without approving live submit."""
from __future__ import annotations

import argparse
import hashlib
import json
from decimal import Decimal, InvalidOperation
from datetime import datetime, timezone
from pathlib import Path

from validate_controlled_canary_external_references import (
    has_placeholder,
    placeholder_paths,
    validate_shape as validate_external_references_shape,
)

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DEFAULT_MANIFEST = ROOT / "evidence" / "current" / "manifest.json"
DEFAULT_APPROVAL = ROOT / "config" / "real-funds-canary.approval.example.json"
DEFAULT_RELEASE_DECISION = ROOT / "config" / "controlled-canary.release-decision.template.json"
DEFAULT_EXTERNAL_REFERENCES = ROOT / "config" / "controlled-canary.external-references.template.json"
DEFAULT_ROOT_CI_RUN_ID = "26268697168"
DEFAULT_HERMES_CI_RUN_ID = "26267887116"
DEFAULT_EXECUTION_ENGINE_CI_RUN_ID = "26268276210"
DEFAULT_CREDENTIALED_SDK_RUN_ID = "local-current-gates-20260523"


def require_sha256(value: str, label: str) -> str:
    if len(value) != 64 or any(ch not in "0123456789abcdefABCDEF" for ch in value):
        raise SystemExit(f"{label} must be a 64-character SHA-256 hex digest")
    return value.lower()


def resolve_input_path(path: Path) -> Path:
    if path.is_absolute() or path.exists():
        return path
    integration_path = INTEGRATION_ROOT / path
    if integration_path.exists():
        return integration_path
    return path


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def positive_decimal_text(value: object) -> bool:
    if not isinstance(value, str) or not value.strip() or value != value.strip():
        return False
    if "REPLACE_WITH" in value:
        return False
    try:
        parsed = Decimal(value)
    except (InvalidOperation, ValueError):
        return False
    return parsed.is_finite() and parsed > 0


def validate_candidate_market_json(candidate_market_bytes: bytes, label: str) -> None:
    try:
        candidate = json.loads(candidate_market_bytes)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{label}: candidate market JSON is invalid: {exc}") from exc
    if not isinstance(candidate, dict):
        raise SystemExit(f"{label}: candidate market must be a JSON object")
    expected_keys = {
        "market_id",
        "token_id",
        "side",
        "order_type",
        "active",
        "accepting_orders",
        "closed",
        "archived",
        "best_ask",
        "limit_price",
        "post_only",
        "ask_size",
        "target_size",
        "spread_bps",
        "min_order_size",
        "exchange_rule_snapshot",
        "liquidity_score",
        "book_snapshot_timestamp",
        "human_review_ref",
    }
    extra = sorted(set(candidate) - expected_keys)
    missing = sorted(expected_keys - set(candidate))
    if extra or missing:
        raise SystemExit(
            f"{label}: candidate market keys mismatch; missing={missing} extra={extra}"
        )
    if candidate.get("side") != "BUY":
        raise SystemExit(f"{label}: candidate market side must be BUY")
    if candidate.get("order_type") != "GTC":
        raise SystemExit(f"{label}: candidate market order_type must be GTC")
    if candidate.get("post_only") is not True:
        raise SystemExit(f"{label}: candidate market post_only must be true")
    if not positive_decimal_text(candidate.get("limit_price")):
        raise SystemExit(f"{label}: candidate market limit_price must be positive")
    if not positive_decimal_text(candidate.get("target_size")):
        raise SystemExit(f"{label}: candidate market target_size must be a concrete positive share size")
    snapshot = candidate.get("exchange_rule_snapshot")
    if not isinstance(snapshot, dict):
        raise SystemExit(f"{label}: candidate market exchange_rule_snapshot must be an object")
    for key in [
        "order_mode",
        "order_type",
        "side",
        "target_size_semantics",
        "captured_at",
        "expires_at",
        "evidence_ref",
    ]:
        value = snapshot.get(key)
        if not isinstance(value, str) or not value.strip() or "REPLACE_WITH" in value:
            raise SystemExit(f"{label}: exchange_rule_snapshot {key} must be concrete")
    if snapshot.get("order_mode") != "post_only_limit" or snapshot.get("order_type") != "GTC":
        raise SystemExit(f"{label}: exchange_rule_snapshot must bind post_only_limit/GTC semantics")
    if candidate.get("active") is not True or candidate.get("accepting_orders") is not True:
        raise SystemExit(f"{label}: candidate market must be active and accepting orders")
    if candidate.get("closed") is not False or candidate.get("archived") is not False:
        raise SystemExit(f"{label}: candidate market must not be closed or archived")
    for key in ["market_id", "token_id", "book_snapshot_timestamp", "human_review_ref"]:
        value = candidate.get(key)
        if not isinstance(value, str) or not value.strip() or "REPLACE_WITH" in value:
            raise SystemExit(f"{label}: candidate market {key} must be concrete")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--approval-template", type=Path, default=DEFAULT_APPROVAL)
    parser.add_argument("--release-decision-template", type=Path, default=DEFAULT_RELEASE_DECISION)
    parser.add_argument("--external-references-template", type=Path, default=DEFAULT_EXTERNAL_REFERENCES)
    parser.add_argument(
        "--external-references-file",
        type=Path,
        help="Use a concrete reviewed reference-only external reference file. Placeholders are rejected.",
    )
    parser.add_argument("--root-ci-run-id", default=DEFAULT_ROOT_CI_RUN_ID)
    parser.add_argument("--hermes-ci-run-id", default=DEFAULT_HERMES_CI_RUN_ID)
    parser.add_argument("--execution-engine-ci-run-id", default=DEFAULT_EXECUTION_ENGINE_CI_RUN_ID)
    parser.add_argument("--credentialed-sdk-run-id", default=DEFAULT_CREDENTIALED_SDK_RUN_ID)
    parser.add_argument(
        "--artifact-sha256",
        help=(
            "Override the artifact hash recorded in the review package. Use this when "
            "the canonical manifest contains source-candidate evidence but an external "
            "release sidecar binds the final zip hash."
        ),
    )
    parser.add_argument(
        "--evidence-manifest-sha256",
        help=(
            "Legacy alias for --archived-evidence-manifest-sha256. This is the "
            "package/sidecar-normalized manifest hash used by armed CLI binding."
        ),
    )
    parser.add_argument(
        "--workspace-evidence-manifest-sha256",
        help="Override the raw workspace evidence/current/manifest.json SHA-256.",
    )
    parser.add_argument(
        "--archived-evidence-manifest-sha256",
        help="Override the normalized manifest SHA-256 recorded in the release artifact sidecar.",
    )
    parser.add_argument(
        "--candidate-market-file",
        type=Path,
        help=(
            "Use a concrete reviewed candidate-market.json instead of the placeholder "
            "template. The file is copied verbatim and its SHA-256 is bound into "
            "approval/release-decision/review metadata."
        ),
    )
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    manifest_path = resolve_input_path(args.manifest)
    approval_template = resolve_input_path(args.approval_template)
    release_decision_template = resolve_input_path(args.release_decision_template)
    external_references_template = resolve_input_path(args.external_references_template)
    external_references_file = (
        resolve_input_path(args.external_references_file)
        if args.external_references_file
        else None
    )
    candidate_market_file = (
        resolve_input_path(args.candidate_market_file)
        if args.candidate_market_file
        else None
    )

    manifest = json.loads(manifest_path.read_text())
    artifact = manifest.get("artifact", {})
    artifact_sha = args.artifact_sha256 or artifact.get("sha256")
    if not artifact_sha:
        raise SystemExit(
            "current manifest does not bind an artifact sha256; pass --artifact-sha256 "
            "from the external release .zip.sha256 sidecar"
        )
    artifact_sha = require_sha256(artifact_sha, "artifact sha256")
    workspace_manifest_sha = require_sha256(
        args.workspace_evidence_manifest_sha256 or sha256(manifest_path),
        "workspace evidence manifest sha256",
    )
    archived_manifest_sha = require_sha256(
        args.archived_evidence_manifest_sha256
        or args.evidence_manifest_sha256
        or workspace_manifest_sha,
        "archived evidence manifest sha256",
    )

    if candidate_market_file:
        candidate_market_bytes = candidate_market_file.read_bytes()
        validate_candidate_market_json(candidate_market_bytes, str(candidate_market_file))
        candidate_market_source = str(candidate_market_file)
    else:
        candidate_market = {
            "active": False,
            "accepting_orders": False,
            "archived": False,
            "ask_size": "0",
            "best_ask": "0",
            "limit_price": "0",
            "post_only": True,
            "book_snapshot_timestamp": "2099-01-01T00:00:00Z",
            "closed": True,
            "human_review_ref": "REPLACE_WITH_OPERATOR_MARKET_REVIEW_REFERENCE",
            "liquidity_score": 0,
            "market_id": "REPLACE_WITH_REVIEWED_CONDITION_ID",
            "min_order_size": "0",
            "exchange_rule_snapshot": {
                "captured_at": "2099-01-01T00:00:00Z",
                "evidence_ref": "REPLACE_WITH_RULE_EVIDENCE_REFERENCE",
                "expires_at": "2099-01-01T00:15:00Z",
                "min_share_size": "REPLACE_WITH_REVIEWED_MIN_SHARE_SIZE",
                "min_tick_size": "REPLACE_WITH_REVIEWED_MIN_TICK_SIZE",
                "order_mode": "post_only_limit",
                "order_type": "GTC",
                "schema_version": 1,
                "side": "BUY",
                "source": "REPLACE_WITH_RULE_SOURCE",
                "target_size_semantics": "outcome_shares",
                "venue": "polymarket_clob",
            },
            "order_type": "GTC",
            "side": "BUY",
            "spread_bps": 2**64 - 1,
            "target_size": "REPLACE_WITH_REVIEWED_TARGET_SHARE_SIZE",
            "token_id": "REPLACE_WITH_REVIEWED_CLOB_TOKEN_ID",
        }
        candidate_market_bytes = (
            json.dumps(candidate_market, indent=2, sort_keys=True) + "\n"
        ).encode()
        candidate_market_source = "placeholder"
    candidate_market_sha = hashlib.sha256(candidate_market_bytes).hexdigest()

    approval = json.loads(approval_template.read_text())
    approval["artifact_sha256"] = artifact_sha
    approval["evidence_manifest_sha256"] = archived_manifest_sha
    approval["workspace_manifest_sha256"] = workspace_manifest_sha
    approval["archived_manifest_sha256"] = archived_manifest_sha
    approval["market_candidate_sha256"] = candidate_market_sha

    release_decision = json.loads(release_decision_template.read_text())
    release_decision["artifact_sha256"] = artifact_sha
    release_decision["evidence_manifest_sha256"] = archived_manifest_sha
    release_decision["workspace_manifest_sha256"] = workspace_manifest_sha
    release_decision["archived_manifest_sha256"] = archived_manifest_sha
    release_decision["market_candidate_sha256"] = candidate_market_sha
    release_decision["operator_identity_ref"] = approval["operator_identity_ref"]
    release_decision["allow_real_funds_canary"] = bool(
        release_decision.get("real_funds_canary_authorized")
    )
    release_decision["github_evidence"] = {
        "root_ci_run_id": args.root_ci_run_id,
        "hermes_ci_run_id": args.hermes_ci_run_id,
        "execution_engine_ci_run_id": args.execution_engine_ci_run_id,
        "credentialed_sdk_run_id": args.credentialed_sdk_run_id,
    }
    external_references_source = external_references_file or external_references_template
    external_references = json.loads(external_references_source.read_text())
    external_references["artifact_sha256"] = artifact_sha
    external_references["evidence_manifest_sha256"] = archived_manifest_sha
    external_references["workspace_manifest_sha256"] = workspace_manifest_sha
    external_references["archived_manifest_sha256"] = archived_manifest_sha
    external_references["github_evidence"] = release_decision["github_evidence"]
    external_reference_failures = validate_external_references_shape(
        external_references,
        str(external_references_source),
        allow_placeholders=external_references_file is None,
    )
    if external_reference_failures:
        raise SystemExit(
            "external references validation failed: "
            + "; ".join(external_reference_failures)
        )
    if external_references_file and has_placeholder(external_references):
        raise SystemExit("external references file must not contain REPLACE_WITH_* placeholders")
    release_decision["external_references"] = {
        "secret_custody_ref": external_references.get("secret_custody", {}).get("provider_ref"),
        "operator_approval_ref": external_references.get("operator_approval", {}).get("ticket_ref"),
        "alert_routing_ref": external_references.get("alert_routing", {}).get("route_ref"),
        "dashboard_ref": external_references.get("alert_routing", {}).get("dashboard_ref"),
        "rollback_runbook_ref": external_references.get("runbooks", {}).get("rollback_runbook_ref"),
        "incident_runbook_ref": external_references.get("runbooks", {}).get("incident_runbook_ref"),
    }

    out = args.output_dir
    out.mkdir(parents=True, exist_ok=True)
    (out / "approval.json").write_text(json.dumps(approval, indent=2, sort_keys=True) + "\n")
    (out / "external-references.json").write_text(
        json.dumps(external_references, indent=2, sort_keys=True) + "\n"
    )
    (out / "release-decision.json").write_text(
        json.dumps(release_decision, indent=2, sort_keys=True) + "\n"
    )
    (out / "candidate-market.json").write_bytes(candidate_market_bytes)

    dry_run_command = [
        "cargo run --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml",
        "--features live-submit --bin pmx-real-funds-canary --",
        "--dry-run",
        "--approval-file approval.json",
        f"--artifact-sha256 {artifact_sha}",
        f"--evidence-manifest-sha256 {archived_manifest_sha}",
        "--idempotency-key dry-run-<UTC_TIMESTAMP>",
        "--account-id acct-canary",
        "--execution-id exec-canary-dry-run-<UTC_TIMESTAMP>",
        "--plan-hash plan-canary-dry-run-<UTC_TIMESTAMP>",
        "--market-file candidate-market.json",
    ]
    review = {
        "schema_version": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "status": "review_package_only_not_armed_approval",
        "artifact_sha256": artifact_sha,
        "evidence_manifest_sha256": archived_manifest_sha,
        "workspace_manifest_sha256": workspace_manifest_sha,
        "archived_manifest_sha256": archived_manifest_sha,
        "market_candidate_sha256": candidate_market_sha,
        "github_evidence": release_decision["github_evidence"],
        "canonical_evidence_manifest": "polymarket-execution-engine/evidence/current/manifest.json",
        "dry_run_command": " ".join(dry_run_command),
        "release_decision_json": "release-decision.json",
        "external_references_json": "external-references.json",
        "external_references_source": str(external_references_source),
        "external_references_placeholders_remaining": placeholder_paths(external_references),
        "candidate_market_source": candidate_market_source,
        "required_before_armed": [
            "reviewed release decision JSON bound to artifact and evidence manifest",
            "complete external references with no placeholders and no secret values",
            "externally selected candidate-market.json prepared from read-only release-prep tooling and validated by execution-engine dry-run",
            "balance and allowance check",
            "explicit --armed operator command",
        ],
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "secrets_included": False,
    }
    (out / "review.json").write_text(json.dumps(review, indent=2, sort_keys=True) + "\n")
    decision_is_go = release_decision.get("decision") == "go"
    readme_status = (
        "This package is a reviewed-go armed canary candidate. It authorizes only the single scoped attempt described in release-decision.json and must be marked consumed/closed after use."
        if decision_is_go
        else "This package is local no-go review material only. It is not an armed approval."
    )
    (out / "README.md").write_text(
        "\n".join(
            [
                "# Real Funds Canary Review Package",
                "",
                readme_status,
                "",
                f"- artifact_sha256: `{artifact_sha}`",
                f"- evidence_manifest_sha256: `{archived_manifest_sha}`",
                f"- workspace_manifest_sha256: `{workspace_manifest_sha}`",
                f"- archived_manifest_sha256: `{archived_manifest_sha}`",
                f"- live_submit_allowed: `{str(bool(release_decision.get('live_submit_authorized'))).lower()}`",
                f"- live_cancel_allowed: `{str(bool(release_decision.get('live_cancel_authorized'))).lower()}`",
                f"- real_funds_canary_authorized: `{str(bool(release_decision.get('real_funds_canary_authorized'))).lower()}`",
                f"- remote_side_effects: `{str(bool(release_decision.get('remote_side_effects_authorized'))).lower()}`",
                "- secrets_included: `false`",
                "- external_references_json: `external-references.json`",
                "- candidate_market_json: `candidate-market.json`",
                "- candidate market discovery is outside the execution engine boundary",
                "- from the integration repository root, replace the placeholder candidate with:",
                "  `python scripts/prepare_canary_candidate_market.py --market-url <polymarket-url> --outcome Yes --output /tmp/pmx-canary-review/candidate-market.json --audit-output /tmp/pmx-canary-review/candidate-market.audit.json --human-review-ref change-ticket://reviewed-canary-market`",
                "",
            ]
        )
    )
    print(json.dumps({"status": "pass", "output_dir": str(out)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
