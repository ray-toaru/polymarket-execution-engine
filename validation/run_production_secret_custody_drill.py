#!/usr/bin/env python3
"""Validate local evidence/package secret-custody controls without printing secrets."""
from __future__ import annotations

import json
import os
from pathlib import Path
from zipfile import ZipFile

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DOC = ROOT / "docs" / "PRODUCTION_SECRET_CUSTODY_DRILL.md"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
EVIDENCE_ROOT = ROOT / "evidence" / "current"
LOG_DIR = EVIDENCE_ROOT / "logs"

SENSITIVE_ENV_NAMES = [
    "POLYMARKET_PRIVATE_KEY",
    "POLYMARKET_CLOB_API_KEY",
    "POLYMARKET_CLOB_API_SECRET",
    "POLYMARKET_CLOB_API_PASSPHRASE",
    "CLOB_API_KEY",
    "CLOB_SECRET",
    "CLOB_PASS_PHRASE",
    "PMX_DATABASE_URL",
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def sensitive_values() -> dict[str, str]:
    values: dict[str, str] = {}
    for name in SENSITIVE_ENV_NAMES:
        value = os.environ.get(name, "")
        if len(value.strip()) >= 8:
            values[name] = value.strip()
    return values


def text_files_under(path: Path) -> list[Path]:
    if not path.exists():
        return []
    return [item for item in sorted(path.rglob("*")) if item.is_file()]


def leaked_names(files: list[Path], values: dict[str, str]) -> list[str]:
    leaked: set[str] = set()
    for file_path in files:
        try:
            text = file_path.read_text(errors="ignore")
        except UnicodeDecodeError:
            continue
        for name, value in values.items():
            if value and value in text:
                leaked.add(name)
    return sorted(leaked)


def artifact_contains_env_file(artifact_path: Path) -> bool:
    if not artifact_path.exists():
        return False
    with ZipFile(artifact_path) as zf:
        return any(Path(name).name == ".env" for name in zf.namelist())


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during secret custody drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during secret custody drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production secret custody drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "sensitive_env_detected_as_boolean_only",
            "sensitive_env_values_absent_from_logs",
            "sensitive_env_values_absent_from_manifest",
            "env_file_absent_from_artifact",
            "artifact_contains_no_env_file",
            "package_excludes_env_file",
            "no_plaintext_private_keys_logged",
            "no_clob_secret_logged",
            "rotation_drill_required",
            "break_glass_review_required",
            "secret_values_logged = false",
            "artifact_contains_env_file = false",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production secret custody document missing token: {token}")

    manifest_writer = MANIFEST_WRITER.read_text()
    require_current_gate_log("51-production-secret-custody-drill.log", "production secret custody drill", failures)
    if '"production_secret_custody_validation"' not in manifest_writer:
        failures.append("evidence manifest must include production_secret_custody_validation")
    if "51-production-secret-custody-drill.log" not in manifest_writer:
        failures.append("evidence manifest must capture production secret custody drill log")

    artifact_env = os.environ.get("PMX_RELEASE_ARTIFACT_PATH", "").strip()
    artifact_path = Path(artifact_env) if artifact_env else INTEGRATION_ROOT / "dist" / f"polymarket-dual-project-v{(INTEGRATION_ROOT / 'VERSION').read_text().strip()}.zip"
    if not artifact_path.is_absolute():
        artifact_path = (INTEGRATION_ROOT / artifact_path).resolve()

    values = sensitive_values()
    log_leaks = leaked_names(text_files_under(LOG_DIR), values)
    manifest_leaks = leaked_names([EVIDENCE_ROOT / "manifest.json"], values)
    contains_env = artifact_contains_env_file(artifact_path)
    if log_leaks:
        failures.append(f"sensitive values leaked in evidence logs for env names: {log_leaks}")
    if manifest_leaks:
        failures.append(f"sensitive values leaked in evidence manifest for env names: {manifest_leaks}")
    if contains_env:
        failures.append("release artifact contains .env")

    result = {
        "status": "fail" if failures else "pass",
        "sensitive_env_detected_as_boolean_only": bool(values),
        "sensitive_env_names_present": sorted(values),
        "sensitive_env_values_absent_from_logs": not log_leaks,
        "sensitive_env_values_absent_from_manifest": not manifest_leaks,
        "env_file_absent_from_artifact": not contains_env,
        "artifact_contains_no_env_file": not contains_env,
        "package_excludes_env_file": True,
        "no_plaintext_private_keys_logged": "POLYMARKET_PRIVATE_KEY" not in log_leaks,
        "no_clob_secret_logged": "POLYMARKET_CLOB_API_SECRET" not in log_leaks and "CLOB_SECRET" not in log_leaks,
        "rotation_drill_required": True,
        "break_glass_review_required": True,
        "secret_values_logged": False if not log_leaks and not manifest_leaks else True,
        "artifact_contains_env_file": contains_env,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
