#!/usr/bin/env python3
"""Validate local deployment preflight artifact/evidence binding."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DOC = ROOT / "docs" / "PRODUCTION_DEPLOYMENT_PREFLIGHT_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during deployment preflight drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during deployment preflight drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production deployment preflight drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "artifact_sha256_verified",
            "artifact_sidecar_verified",
            "evidence_sidecar_verified",
            "evidence_manifest_sha256_bound",
            "migration_evidence_present",
            "config_diff_review_required",
            "operator_approval_required",
            "live_submit_disabled",
            "live_cancel_disabled",
            "deploy_allowed = false",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production deployment preflight document missing token: {token}")

    manifest_writer = MANIFEST.read_text()
    require_current_gate_log(
        "50-production-deployment-preflight-drill.log",
        "production deployment preflight drill",
        failures,
    )
    if '"production_deployment_preflight_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_deployment_preflight_validation")
    if "50-production-deployment-preflight-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production deployment preflight log")

    artifact_env = os.environ.get("PMX_RELEASE_ARTIFACT_PATH", "").strip()
    artifact_path = Path(artifact_env) if artifact_env else INTEGRATION_ROOT / "dist" / f"polymarket-dual-project-v{(INTEGRATION_ROOT / 'VERSION').read_text().strip()}.zip"
    if not artifact_path.is_absolute():
        artifact_path = (INTEGRATION_ROOT / artifact_path).resolve()

    artifact_sha = None
    sidecar_ok = False
    evidence_sidecar_ok = False
    evidence_manifest_sha_ok = False
    sidecar_path = artifact_path.with_suffix(artifact_path.suffix + ".sha256")
    evidence_sidecar_path = artifact_path.with_suffix(artifact_path.suffix + ".evidence.json")
    if not artifact_path.exists():
        failures.append(f"release artifact missing: {artifact_path}")
    else:
        artifact_sha = sha256(artifact_path)
        if not sidecar_path.exists():
            failures.append(f"artifact sha256 sidecar missing: {sidecar_path}")
        else:
            sidecar_ok = sidecar_path.read_text().startswith(f"{artifact_sha}  {artifact_path.name}")
            if not sidecar_ok:
                failures.append("artifact sha256 sidecar does not match artifact")
        if not evidence_sidecar_path.exists():
            failures.append(f"artifact evidence sidecar missing: {evidence_sidecar_path}")
        else:
            evidence_sidecar = json.loads(evidence_sidecar_path.read_text())
            evidence_sidecar_ok = evidence_sidecar.get("artifact", {}).get("sha256") == artifact_sha
            if not evidence_sidecar_ok:
                failures.append("artifact evidence sidecar does not bind artifact hash")
            manifest_rel = evidence_sidecar.get("canonical_evidence", {}).get("manifest_path")
            manifest_sha = evidence_sidecar.get("canonical_evidence", {}).get("manifest_sha256")
            manifest_path = INTEGRATION_ROOT / str(manifest_rel)
            evidence_manifest_sha_ok = manifest_path.exists() and sha256(manifest_path) == manifest_sha
            if not evidence_manifest_sha_ok:
                failures.append("artifact evidence sidecar does not bind current evidence manifest hash")

    for log_name in [
        "13-pg-migration.log",
        "27-package-release.log",
        "28-release-artifact-check.log",
    ]:
        require_current_gate_log(log_name, f"deployment preflight source evidence {log_name}", failures)

    result = {
        "status": "fail" if failures else "pass",
        "artifact": {
            "path": str(artifact_path),
            "sha256": artifact_sha,
            "artifact_sha256_verified": artifact_sha is not None,
            "artifact_sidecar_verified": sidecar_ok,
            "evidence_sidecar_verified": evidence_sidecar_ok,
            "evidence_manifest_sha256_bound": evidence_manifest_sha_ok,
        },
        "migration_evidence_present": True,
        "config_diff_review_required": True,
        "operator_approval_required": True,
        "live_submit_disabled": not env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_disabled": not env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "deploy_allowed": False,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
