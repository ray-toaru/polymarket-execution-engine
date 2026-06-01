#!/usr/bin/env python3
"""Build a self-contained reviewed-go package from approved dual-control material."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
REVIEWED_GO_DECISION = INTEGRATION_ROOT / "scripts" / "prepare_reviewed_go_decision.py"
REVIEW_PACKET = INTEGRATION_ROOT / "scripts" / "prepare_dual_control_review_packet.py"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def resolve(path: Path) -> Path:
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def require_file(path: Path, label: str) -> Path:
    if not path.is_file():
        raise SystemExit(f"{label} missing: {path}")
    return path


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def copy_into_package(src: Path, output_dir: Path, *, target_name: str | None = None) -> dict[str, str]:
    dest = output_dir / (target_name or src.name)
    shutil.copy2(src, dest)
    return {"path": dest.name, "sha256": sha256(dest)}


def build_cli_approval(approval_request: dict[str, Any]) -> dict[str, Any]:
    account_id = approval_request.get("account_id")
    if not isinstance(account_id, str) or not account_id.strip():
        raise SystemExit("approval request account_id is required to build canonical approval.json")
    risk_limits = approval_request.get("risk_limits")
    if not isinstance(risk_limits, dict):
        raise SystemExit("approval request risk_limits must be an object")
    condition_id = approval_request.get("condition_id")
    if not isinstance(condition_id, str) or not condition_id.strip():
        raise SystemExit("approval request condition_id is required to build canonical approval.json")
    runtime_gate_snapshot = approval_request.get("runtime_gate_snapshot")
    if not isinstance(runtime_gate_snapshot, dict):
        raise SystemExit("approval request runtime_gate_snapshot must be an object to build canonical approval.json")
    runtime_gate_evidence_refs = approval_request.get("runtime_gate_evidence_refs")
    if not isinstance(runtime_gate_evidence_refs, dict):
        raise SystemExit("approval request runtime_gate_evidence_refs must be an object to build canonical approval.json")
    return {
        "approval_id": approval_request["approval_id"],
        "approval_hash": approval_request["approval_hash"],
        "account_id": account_id.strip(),
        "condition_id": condition_id.strip(),
        "scope": approval_request["scope"],
        "expires_at": approval_request["expires_at"],
        "artifact_sha256": approval_request["artifact_sha256"],
        "evidence_manifest_sha256": approval_request["evidence_manifest_sha256"],
        "workspace_manifest_sha256": approval_request["workspace_manifest_sha256"],
        "archived_manifest_sha256": approval_request["archived_manifest_sha256"],
        "market_candidate_sha256": approval_request["market_candidate_sha256"],
        "max_order_notional_usd": risk_limits["max_order_notional_usd"],
        "max_daily_notional_usd": risk_limits["max_daily_notional_usd"],
        "execution_style": approval_request["execution_style"],
        "operator_identity_ref": approval_request["operator_identity_ref"],
        "operator_identity_sha256": approval_request["operator_identity_sha256"],
        "runtime_gate_snapshot": runtime_gate_snapshot,
        "runtime_gate_evidence_refs": runtime_gate_evidence_refs,
    }


def build_package(
    *,
    output_dir: Path,
    release_zip: Path,
    release_sha: Path,
    release_evidence: Path,
    candidate_market: Path,
    runtime_truth: Path,
    approval_request: Path,
    dual_control_review: Path,
    external_references: Path,
    decision_id: str,
    decision_reason: str,
) -> dict[str, Any]:
    reviewed_go = load_module(REVIEWED_GO_DECISION, "prepare_reviewed_go_decision")
    review_packet = load_module(REVIEW_PACKET, "prepare_dual_control_review_packet")

    sidecar = review_packet.validate_release_sidecar(release_evidence)
    review_packet.validate_candidate(candidate_market)
    runtime_doc = review_packet.validate_runtime_truth(runtime_truth)
    approval_doc = review_packet.validate_approval_request(approval_request)
    cli_approval = build_cli_approval(approval_doc)

    candidate_sha = sha256(candidate_market)
    runtime_sha = sha256(runtime_truth)
    approval_sha = reviewed_go.require_sha256(sha256(approval_request), "approval request file sha256")
    review_sha = reviewed_go.require_sha256(sha256(dual_control_review), "dual-control review file sha256")

    expected_pairs = [
        ("artifact_sha256", sidecar["artifact_sha256"], approval_doc.get("artifact_sha256")),
        ("artifact_sha256", sidecar["artifact_sha256"], runtime_doc.get("artifact_sha256")),
        ("workspace_manifest_sha256", sidecar["workspace_manifest_sha256"], approval_doc.get("workspace_manifest_sha256")),
        ("workspace_manifest_sha256", sidecar["workspace_manifest_sha256"], runtime_doc.get("workspace_manifest_sha256")),
        ("archived_manifest_sha256", sidecar["archived_manifest_sha256"], approval_doc.get("archived_manifest_sha256")),
        ("archived_manifest_sha256", sidecar["archived_manifest_sha256"], runtime_doc.get("archived_manifest_sha256")),
        ("market_candidate_sha256", candidate_sha, approval_doc.get("market_candidate_sha256")),
        ("runtime_truth_sha256", runtime_sha, approval_doc.get("runtime_truth_sha256")),
        ("condition_id", runtime_doc.get("condition_id"), approval_doc.get("condition_id")),
    ]
    mismatches = [
        f"{field} expected {expected}, got {actual!r}"
        for field, expected, actual in expected_pairs
        if expected != actual
    ]
    if mismatches:
        raise SystemExit("reviewed-go package binding mismatch: " + "; ".join(mismatches))
    runtime_gate_snapshot = approval_doc.get("runtime_gate_snapshot")
    runtime_report = runtime_doc.get("preflight_report")
    if not isinstance(runtime_gate_snapshot, dict) or not isinstance(runtime_report, dict):
        raise SystemExit("reviewed-go package requires runtime gate snapshots in approval request and runtime truth")
    gate_mismatches = [
        field
        for field, value in runtime_gate_snapshot.items()
        if runtime_report.get(field) is not value
    ]
    if gate_mismatches:
        raise SystemExit(
            "reviewed-go package runtime gate snapshot mismatch: " + ", ".join(sorted(gate_mismatches))
        )
    approval_gate_evidence_refs = approval_doc.get("runtime_gate_evidence_refs")
    runtime_gate_evidence_refs = runtime_report.get("gate_evidence_refs")
    if not isinstance(approval_gate_evidence_refs, dict) or not isinstance(runtime_gate_evidence_refs, dict):
        raise SystemExit("reviewed-go package requires runtime gate evidence refs in approval request and runtime truth")
    evidence_mismatches = [
        field
        for field, value in approval_gate_evidence_refs.items()
        if runtime_gate_evidence_refs.get(field) != value
    ]
    if evidence_mismatches:
        raise SystemExit(
            "reviewed-go package runtime gate evidence mismatch: " + ", ".join(sorted(evidence_mismatches))
        )

    decision = reviewed_go.build_decision(
        approval_doc,
        load_json(external_references),
        decision_id=decision_id,
        decision_reason=decision_reason,
        dual_control_review=load_json(dual_control_review),
        dual_control_review_sha256=review_sha,
        approval_request_sha256=approval_sha,
    )
    reviewed_go.validate_decision_output(decision)

    output_dir.mkdir(parents=True, exist_ok=True)
    copied = {
        "release_zip": copy_into_package(release_zip, output_dir),
        "release_sha256": copy_into_package(release_sha, output_dir),
        "release_evidence": copy_into_package(release_evidence, output_dir),
        "candidate_market": copy_into_package(candidate_market, output_dir),
        "runtime_truth": copy_into_package(runtime_truth, output_dir),
        "approval_request": copy_into_package(
            approval_request,
            output_dir,
            target_name="approval-request.json",
        ),
        "dual_control_review": copy_into_package(dual_control_review, output_dir, target_name="dual-control-review.json"),
        "external_references": copy_into_package(external_references, output_dir),
    }
    approval_path = output_dir / "approval.json"
    approval_path.write_text(json.dumps(cli_approval, indent=2, sort_keys=True) + "\n")
    copied["approval"] = {"path": "approval.json", "sha256": sha256(approval_path)}
    (output_dir / "release-decision.json").write_text(json.dumps(decision, indent=2, sort_keys=True) + "\n")
    copied["release_decision"] = {"path": "release-decision.json", "sha256": sha256(output_dir / "release-decision.json")}

    review = {
        "schema_version": 1,
        "status": "reviewed_go_package_ready_single_attempt",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "decision_id": decision_id,
        "decision_reason": decision_reason,
        "scope": "REAL_FUNDS_CANARY",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "artifact_sha256": sidecar["artifact_sha256"],
        "workspace_manifest_sha256": sidecar["workspace_manifest_sha256"],
        "archived_manifest_sha256": sidecar["archived_manifest_sha256"],
        "candidate_market_sha256": candidate_sha,
        "runtime_truth_sha256": runtime_sha,
        "approval_hash": approval_doc["approval_hash"],
        "active_profile_ref": approval_doc.get("active_profile_ref"),
        "approval_request_sha256": approval_sha,
        "dual_control_review_sha256": review_sha,
        "live_submit_authorized": True,
        "live_cancel_authorized": True,
        "remote_side_effects_authorized": True,
        "production_deployment_authorized": False,
        "single_attempt_only": True,
        "operator_reuse_after_consumption_allowed": False,
        "files": copied,
    }
    (output_dir / "review.json").write_text(json.dumps(review, indent=2, sort_keys=True) + "\n")
    (output_dir / "README.md").write_text(package_readme(review))
    return review


def package_readme(review: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Reviewed Go Canary Package",
            "",
            "This directory is a reviewed-go package for one controlled canary attempt.",
            "It is authorization-bearing local material and must be marked consumed and then closed after use.",
            "",
            f"- Package status: `{review['status']}`",
            f"- Artifact SHA-256: `{review['artifact_sha256']}`",
            f"- Candidate SHA-256: `{review['candidate_market_sha256']}`",
            f"- Runtime-truth SHA-256: `{review['runtime_truth_sha256']}`",
            f"- Approval hash: `{review['approval_hash']}`",
            f"- Active profile ref: `{review.get('active_profile_ref')}`",
            f"- Approval request SHA-256: `{review['approval_request_sha256']}`",
            f"- Dual-control review SHA-256: `{review['dual_control_review_sha256']}`",
            "",
            "Included files:",
            "",
            "- `release-decision.json`",
            "- `approval.json` (canonical CLI approval)",
            "- `approval-request.json` (governance request evidence)",
            "- `dual-control-review.json`",
            "- `external-references.json`",
            "- `candidate-market.json`",
            "- `runtime-truth.json`",
            "- release zip and detached sidecars",
            "",
            "Operator constraints:",
            "",
            "- single attempt only",
            "- live submit and live cancel authorized only within the bound scope",
            "- production deployment remains unauthorized",
            "- after any armed run, record consumption and closeout before any later review",
            "",
        ]
    ) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--release-zip", required=True, type=Path)
    parser.add_argument("--candidate-market-file", required=True, type=Path)
    parser.add_argument("--runtime-truth-file", required=True, type=Path)
    parser.add_argument("--approval-request-file", required=True, type=Path)
    parser.add_argument("--dual-control-review-file", required=True, type=Path)
    parser.add_argument("--external-references-file", required=True, type=Path)
    parser.add_argument("--decision-id", required=True)
    parser.add_argument("--decision-reason", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    output_dir = resolve(args.output_dir)
    release_zip = require_file(resolve(args.release_zip), "release zip")
    release_sha = require_file(release_zip.with_suffix(release_zip.suffix + ".sha256"), "release sha sidecar")
    release_evidence = require_file(
        release_zip.with_suffix(release_zip.suffix + ".evidence.json"),
        "release evidence sidecar",
    )
    review = build_package(
        output_dir=output_dir,
        release_zip=release_zip,
        release_sha=release_sha,
        release_evidence=release_evidence,
        candidate_market=require_file(resolve(args.candidate_market_file), "candidate market"),
        runtime_truth=require_file(resolve(args.runtime_truth_file), "runtime truth"),
        approval_request=require_file(resolve(args.approval_request_file), "approval request"),
        dual_control_review=require_file(resolve(args.dual_control_review_file), "dual-control review"),
        external_references=require_file(resolve(args.external_references_file), "external references"),
        decision_id=args.decision_id,
        decision_reason=args.decision_reason,
    )
    print(json.dumps({"status": "pass", "output_dir": str(output_dir), "package_status": review["status"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
