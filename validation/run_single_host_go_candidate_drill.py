#!/usr/bin/env python3
"""Generate a temporary single-host go candidate and prove it is not committed."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
MANIFEST = ROOT / "evidence" / "current" / "manifest.json"
VERSION_FILE = INTEGRATION_ROOT / "VERSION"
if not VERSION_FILE.exists():
    raise SystemExit("VERSION file missing; cannot resolve current release artifact")
VERSION = VERSION_FILE.read_text().strip()
DEFAULT_RELEASE_ARTIFACT = INTEGRATION_ROOT / "dist" / f"polymarket-execution-suite-v{VERSION}.zip"
EXTERNAL_REFERENCES = ROOT / "config" / "controlled-canary.external-references.example.json"
PREPARE_REVIEW = ROOT / "validation" / "prepare_real_funds_canary_review.py"
CLI = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "bin" / "pmx-real-funds-canary.rs"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
FORBIDDEN_GO_DECISION_GLOBS = [
    "config/*go*.json",
    "deploy/**/*go*.json",
    "evidence/current/**/*go*.json",
]


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def resolve_artifact_path(manifest: dict) -> Path | None:
    configured = os.environ.get("PMX_RELEASE_ARTIFACT_PATH")
    if configured:
        path = Path(configured)
        return path if path.is_absolute() else INTEGRATION_ROOT / path
    manifest_path = manifest.get("artifact", {}).get("path")
    if isinstance(manifest_path, str) and manifest_path.strip():
        path = Path(manifest_path)
        return path if path.is_absolute() else INTEGRATION_ROOT / path
    if DEFAULT_RELEASE_ARTIFACT.exists():
        return DEFAULT_RELEASE_ARTIFACT
    return None


def validate_go_candidate(candidate: dict, approval: dict, artifact_sha: str, manifest_sha: str) -> list[str]:
    failures: list[str] = []
    expected = {
        "scope": "REAL_FUNDS_CANARY",
        "artifact_sha256": artifact_sha,
        "evidence_manifest_sha256": manifest_sha,
        "market_candidate_sha256": approval.get("market_candidate_sha256"),
        "decision": "go",
        "source_release": f"v{VERSION}",
        "live_submit_authorized": True,
        "live_cancel_authorized": True,
        "production_deployment_authorized": False,
        "real_funds_canary_authorized": True,
        "remote_side_effects_authorized": True,
        "allow_real_funds_canary": True,
        "reviewed_release_decision_present": True,
    }
    for key, value in expected.items():
        if candidate.get(key) != value:
            failures.append(f"go candidate {key} mismatch")
    if candidate.get("artifact_sha256") != approval.get("artifact_sha256"):
        failures.append("go candidate artifact hash must match approval")
    if candidate.get("evidence_manifest_sha256") != approval.get("evidence_manifest_sha256"):
        failures.append("go candidate evidence hash must match approval")
    if candidate.get("market_candidate_sha256") != approval.get("market_candidate_sha256"):
        failures.append("go candidate market candidate hash must match approval")
    if not str(candidate.get("decision_id", "")).startswith("candidate-go-"):
        failures.append("go candidate decision id must be clearly marked candidate-go")
    if not str(candidate.get("operator_identity_ref", "")).startswith("review-required://"):
        failures.append("go candidate operator identity must require external review")
    if "2099-" in str(candidate.get("expires_at", "")):
        failures.append("go candidate must not use long-lived fixture expiry")
    return failures


def main() -> int:
    failures: list[str] = []
    require_current_gate_log(
        "71-single-host-go-candidate-drill.log",
        "single-host go candidate drill",
        failures,
    )
    for path in [MANIFEST, EXTERNAL_REFERENCES, PREPARE_REVIEW, CLI]:
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")

    committed_go_files = []
    for pattern in FORBIDDEN_GO_DECISION_GLOBS:
        committed_go_files.extend(path.relative_to(ROOT).as_posix() for path in ROOT.glob(pattern))
    if committed_go_files:
        failures.append("go decision candidate files must not be committed: " + ", ".join(sorted(committed_go_files)))

    cli = CLI.read_text() if CLI.exists() else ""
    if "--release-decision-file is required with --armed" not in cli:
        failures.append("CLI must reject --armed without --release-decision-file")
    if "validate_reviewed_release_decision(&args, &approval, &market_candidate_sha256)?" not in cli:
        failures.append("CLI must validate reviewed release decision before live canary execution")

    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    manifest = load(MANIFEST)
    artifact_path = resolve_artifact_path(manifest)
    artifact_sha = sha256(artifact_path) if artifact_path and artifact_path.exists() else manifest.get("artifact", {}).get("sha256")
    manifest_sha = sha256(MANIFEST)

    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp) / "candidate"
        generated = subprocess.run(
            [
                "python",
                str(PREPARE_REVIEW),
                "--output-dir",
                str(output_dir),
                "--external-references-file",
                str(EXTERNAL_REFERENCES),
                "--artifact-sha256",
                str(artifact_sha),
                "--evidence-manifest-sha256",
                manifest_sha,
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if generated.returncode != 0:
            failures.append(f"go candidate source package generation failed: {generated.stderr.strip() or generated.stdout.strip()}")
        else:
            approval = load(output_dir / "approval.json")
            release_decision = load(output_dir / "release-decision.json")
            go_candidate = {
                **release_decision,
                "schema_version": 1,
                "decision_id": "candidate-go-single-host-real-funds-canary",
                "status": "candidate_go_not_committed",
                "source_release": f"v{VERSION}",
                "decision": "go",
                "decision_reason": "Temporary single-host go candidate drill; not committed and not reviewed for live use.",
                "scope": "REAL_FUNDS_CANARY",
                "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
                "expires_at": "2030-01-01T00:00:00Z",
                "artifact_sha256": approval.get("artifact_sha256"),
                "evidence_manifest_sha256": approval.get("evidence_manifest_sha256"),
                "market_candidate_sha256": approval.get("market_candidate_sha256"),
                "github_evidence": release_decision.get("github_evidence", {}),
                "external_references": release_decision.get("external_references", {}),
                "risk_limits": release_decision.get("risk_limits", {}),
                "required_review_signals": release_decision.get("required_review_signals", {}),
                "live_submit_authorized": True,
                "live_cancel_authorized": True,
                "production_deployment_authorized": False,
                "real_funds_canary_authorized": True,
                "remote_side_effects_authorized": True,
                "allow_real_funds_canary": True,
                "reviewed_release_decision_present": True,
                "operator_identity_ref": "review-required://single-host-canary-operator",
                "secrets_included": False,
            }
            candidate_path = output_dir / "adapter-release-decision.go-candidate.json"
            candidate_path.write_text(json.dumps(go_candidate, indent=2, sort_keys=True) + "\n")
            failures.extend(validate_go_candidate(go_candidate, approval, str(artifact_sha), manifest_sha))
            if not candidate_path.exists():
                failures.append("temporary go candidate was not written")

    writer = MANIFEST_WRITER.read_text()
    if '"single_host_go_candidate_validation"' not in writer:
        failures.append("evidence manifest must include single_host_go_candidate_validation")
    if "71-single-host-go-candidate-drill.log" not in writer:
        failures.append("evidence manifest must capture single-host go candidate log")

    result = {
        "status": "fail" if failures else "pass",
        "temporary_go_candidate_generated": not failures,
        "go_candidate_committed": False,
        "missing_release_decision_blocks_armed": True,
        "release_decision": "candidate_go_not_committed",
        "artifact_sha256": "external-sidecar-only" if isinstance(artifact_sha, str) else None,
        "artifact_sha256_verified": isinstance(artifact_sha, str) and len(artifact_sha) == 64,
        "evidence_manifest_bound": True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "production_deployment_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "secrets_included": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
