#!/usr/bin/env python3
"""Prepare a local real-funds canary review package without approving live submit."""
from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DEFAULT_MANIFEST = ROOT / "evidence" / "current" / "manifest.json"
DEFAULT_APPROVAL = ROOT / "config" / "real-funds-canary.approval.example.json"
DEFAULT_RELEASE_DECISION = ROOT / "config" / "controlled-canary.release-decision.template.json"
DEFAULT_ROOT_CI_RUN_ID = "26176061318"
DEFAULT_HERMES_CI_RUN_ID = "26174554396"
DEFAULT_EXECUTION_ENGINE_CI_RUN_ID = "26174564854"
DEFAULT_CREDENTIALED_SDK_RUN_ID = "26175786984"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--approval-template", type=Path, default=DEFAULT_APPROVAL)
    parser.add_argument("--release-decision-template", type=Path, default=DEFAULT_RELEASE_DECISION)
    parser.add_argument("--root-ci-run-id", default=DEFAULT_ROOT_CI_RUN_ID)
    parser.add_argument("--hermes-ci-run-id", default=DEFAULT_HERMES_CI_RUN_ID)
    parser.add_argument("--execution-engine-ci-run-id", default=DEFAULT_EXECUTION_ENGINE_CI_RUN_ID)
    parser.add_argument("--credentialed-sdk-run-id", default=DEFAULT_CREDENTIALED_SDK_RUN_ID)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text())
    artifact = manifest.get("artifact", {})
    artifact_sha = artifact.get("sha256")
    if not artifact_sha:
        raise SystemExit("current manifest does not bind an artifact sha256")
    manifest_sha = sha256(args.manifest)

    approval = json.loads(args.approval_template.read_text())
    approval["artifact_sha256"] = artifact_sha
    approval["evidence_manifest_sha256"] = manifest_sha

    release_decision = json.loads(args.release_decision_template.read_text())
    release_decision["artifact_sha256"] = artifact_sha
    release_decision["evidence_manifest_sha256"] = manifest_sha
    release_decision["github_evidence"] = {
        "root_ci_run_id": args.root_ci_run_id,
        "hermes_ci_run_id": args.hermes_ci_run_id,
        "execution_engine_ci_run_id": args.execution_engine_ci_run_id,
        "credentialed_sdk_run_id": args.credentialed_sdk_run_id,
    }

    out = args.output_dir
    out.mkdir(parents=True, exist_ok=True)
    (out / "approval.json").write_text(json.dumps(approval, indent=2, sort_keys=True) + "\n")
    (out / "release-decision.json").write_text(
        json.dumps(release_decision, indent=2, sort_keys=True) + "\n"
    )

    dry_run_command = [
        "cargo run --manifest-path adapters/pmx-official-sdk-adapter/Cargo.toml",
        "--features live-submit --bin pmx-real-funds-canary --",
        "--dry-run",
        "--approval-file approval.json",
        f"--artifact-sha256 {artifact_sha}",
        f"--evidence-manifest-sha256 {manifest_sha}",
        "--idempotency-key dry-run-<UTC_TIMESTAMP>",
        "--account-id acct-canary",
        "--execution-id exec-canary-dry-run-<UTC_TIMESTAMP>",
        "--plan-hash plan-canary-dry-run-<UTC_TIMESTAMP>",
    ]
    review = {
        "schema_version": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "status": "review_package_only_not_armed_approval",
        "artifact_sha256": artifact_sha,
        "evidence_manifest_sha256": manifest_sha,
        "github_evidence": release_decision["github_evidence"],
        "canonical_evidence_manifest": "polymarket-execution-engine/evidence/current/manifest.json",
        "dry_run_command": " ".join(dry_run_command),
        "release_decision_json": "release-decision.json",
        "required_before_armed": [
            "reviewed release decision JSON bound to artifact and evidence manifest",
            "successful dry-run with a safe market candidate",
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
    (out / "README.md").write_text(
        "\n".join(
            [
                "# Real Funds Canary Review Package",
                "",
                "This package is local review material only. It is not an armed approval.",
                "",
                f"- artifact_sha256: `{artifact_sha}`",
                f"- evidence_manifest_sha256: `{manifest_sha}`",
                "- live_submit_allowed: `false`",
                "- live_cancel_allowed: `false`",
                "- real_funds_canary_authorized: `false`",
                "- remote_side_effects: `false`",
                "- secrets_included: `false`",
                "",
            ]
        )
    )
    print(json.dumps({"status": "pass", "output_dir": str(out)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
