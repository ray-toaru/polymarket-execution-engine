#!/usr/bin/env python3
"""Validate a single-host canary candidate package remains no-go and dry-run only."""
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
EXTERNAL_REFERENCES = ROOT / "config" / "controlled-canary.external-references.example.json"
PREPARE_REVIEW = ROOT / "validation" / "prepare_real_funds_canary_review.py"
PACKAGE_PREFLIGHT = ROOT / "deploy" / "single-host" / "bin" / "pmx-single-host-canary-package-preflight.sh"
CANARY_SERVICE = ROOT / "deploy" / "single-host" / "systemd" / "pmx-real-funds-canary@.service"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"


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
    return None


def main() -> int:
    failures: list[str] = []
    require_current_gate_log(
        "70-single-host-canary-candidate-drill.log",
        "single-host canary candidate drill",
        failures,
    )
    for path in [MANIFEST, EXTERNAL_REFERENCES, PREPARE_REVIEW, PACKAGE_PREFLIGHT, CANARY_SERVICE]:
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")
    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    manifest = load(MANIFEST)
    artifact_path = resolve_artifact_path(manifest)
    if artifact_path and artifact_path.exists():
        artifact_sha = sha256(artifact_path)
    else:
        artifact_sha = manifest.get("artifact", {}).get("sha256")
    manifest_sha = sha256(MANIFEST)
    if not isinstance(artifact_sha, str) or len(artifact_sha) != 64:
        failures.append("current manifest artifact.sha256 must be concrete")

    service_text = CANARY_SERVICE.read_text()
    if "--dry-run" not in service_text:
        failures.append("single-host canary service must remain dry-run")
    for forbidden in ["--armed", "--allow-live-submit-config", "--allow-real-funds-canary-config"]:
        if forbidden in service_text:
            failures.append(f"single-host canary service must not include {forbidden}")

    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp) / "candidate"
        command = [
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
        ]
        generated = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
        if generated.returncode != 0:
            failures.append(f"candidate package generation failed: {generated.stderr.strip() or generated.stdout.strip()}")
        else:
            preflight = subprocess.run(
                [str(PACKAGE_PREFLIGHT), str(output_dir)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
                env={**os.environ, "PMX_EXECUTION_ENGINE_ROOT": str(ROOT)},
            )
            if preflight.returncode != 0:
                failures.append(f"candidate package preflight failed: {preflight.stderr.strip() or preflight.stdout.strip()}")
            decision = load(output_dir / "release-decision.json")
            review = load(output_dir / "review.json")
            external = load(output_dir / "external-references.json")
            if decision.get("decision") != "no_go":
                failures.append("candidate release decision must remain no_go")
            for key in [
                "live_submit_authorized",
                "live_cancel_authorized",
                "production_deployment_authorized",
                "real_funds_canary_authorized",
                "remote_side_effects_authorized",
            ]:
                if decision.get(key) is not False:
                    failures.append(f"candidate release decision must keep {key}=false")
            for key in ["live_submit_allowed", "live_cancel_allowed", "real_funds_canary_authorized", "remote_side_effects", "secrets_included"]:
                if review.get(key) is not False:
                    failures.append(f"candidate review must keep {key}=false")
                if key in external and external.get(key) is not False:
                    failures.append(f"candidate external references must keep {key}=false")

    writer = MANIFEST_WRITER.read_text()
    if '"single_host_canary_candidate_validation"' not in writer:
        failures.append("evidence manifest must include single_host_canary_candidate_validation")
    if "70-single-host-canary-candidate-drill.log" not in writer:
        failures.append("evidence manifest must capture single-host canary candidate log")

    result = {
        "status": "fail" if failures else "pass",
        "candidate_package_generated": not failures,
        "release_decision": "no_go",
        "canary_runner_mode": "dry-run",
        "artifact_sha256": artifact_sha,
        "artifact_path": str(artifact_path) if artifact_path else None,
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
