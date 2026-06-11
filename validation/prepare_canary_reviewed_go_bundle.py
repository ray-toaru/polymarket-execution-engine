#!/usr/bin/env python3
"""Promote a dual-control review packet into a reviewed-go package."""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
REVIEWED_GO_PACKAGE_SCRIPT = INTEGRATION_ROOT / "scripts" / "prepare_reviewed_go_package.py"


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def resolve(path: Path) -> Path:
    return path if path.is_absolute() else INTEGRATION_ROOT / path


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review-packet-dir", required=True, type=Path)
    parser.add_argument("--approved-dual-control-review-file", required=True, type=Path)
    parser.add_argument("--canonical-dual-control-review-file", required=True, type=Path)
    parser.add_argument("--review-signature-file", required=True, type=Path)
    parser.add_argument("--reviewer-registry-file", required=True, type=Path)
    parser.add_argument("--external-references-file", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--decision-id")
    parser.add_argument("--decision-reason", default="approved by independent reviewer")
    return parser.parse_args()


def default_decision_id() -> str:
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return f"reviewed-go-{timestamp}"


def require_packet(packet_dir: Path) -> dict:
    packet_path = packet_dir / "packet.json"
    if not packet_path.is_file():
        raise SystemExit(f"review packet missing packet.json: {packet_path}")
    packet = load_json(packet_path)
    if packet.get("status") != "dual_control_review_packet_not_authorization":
        raise SystemExit("review packet status must be dual_control_review_packet_not_authorization")
    files = packet.get("files")
    if not isinstance(files, dict):
        raise SystemExit("review packet files must be an object")
    return packet


def file_from_packet(packet_dir: Path, packet: dict, key: str) -> Path:
    files = packet["files"]
    entry = files.get(key)
    if not isinstance(entry, dict):
        raise SystemExit(f"review packet missing files.{key}")
    rel = entry.get("path")
    if not isinstance(rel, str) or not rel.strip():
        raise SystemExit(f"review packet files.{key}.path is required")
    path = packet_dir / rel
    if not path.is_file():
        raise SystemExit(f"review packet referenced file missing for {key}: {path}")
    return path


def prepare_reviewed_go_bundle(
    *,
    review_packet_dir: Path,
    approved_dual_control_review_file: Path,
    canonical_dual_control_review_file: Path,
    review_signature_file: Path,
    reviewer_registry_file: Path,
    external_references_file: Path,
    output_dir: Path,
    decision_id: str | None,
    decision_reason: str,
) -> dict[str, str]:
    reviewed_go_package = load_module(REVIEWED_GO_PACKAGE_SCRIPT, "prepare_reviewed_go_package")
    packet = require_packet(review_packet_dir)

    review = reviewed_go_package.build_package(
        output_dir=output_dir,
        release_zip=file_from_packet(review_packet_dir, packet, "release_zip"),
        release_sha=file_from_packet(review_packet_dir, packet, "release_sha256"),
        release_evidence=file_from_packet(review_packet_dir, packet, "release_evidence"),
        candidate_market=file_from_packet(review_packet_dir, packet, "candidate_market"),
        runtime_truth=file_from_packet(review_packet_dir, packet, "runtime_truth"),
        approval_request=file_from_packet(review_packet_dir, packet, "approval_request"),
        dual_control_review=approved_dual_control_review_file,
        canonical_dual_control_review=canonical_dual_control_review_file,
        review_signature=review_signature_file,
        reviewer_registry=reviewer_registry_file,
        external_references=external_references_file,
        decision_id=decision_id or default_decision_id(),
        decision_reason=decision_reason,
    )
    return {
        "status": "pass",
        "review_packet_dir": str(review_packet_dir),
        "output_dir": str(output_dir),
        "package_status": review["status"],
        "active_profile_ref": review.get("active_profile_ref") or "",
        "approval_hash": review["approval_hash"],
    }


def main() -> int:
    args = parse_args()
    result = prepare_reviewed_go_bundle(
        review_packet_dir=resolve(args.review_packet_dir),
        approved_dual_control_review_file=resolve(args.approved_dual_control_review_file),
        canonical_dual_control_review_file=resolve(args.canonical_dual_control_review_file),
        review_signature_file=resolve(args.review_signature_file),
        reviewer_registry_file=resolve(args.reviewer_registry_file),
        external_references_file=resolve(args.external_references_file),
        output_dir=resolve(args.output_dir),
        decision_id=args.decision_id,
        decision_reason=args.decision_reason,
    )
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
