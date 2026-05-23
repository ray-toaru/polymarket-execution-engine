#!/usr/bin/env python3
"""Validate the external secret-provider preflight contract without printing secrets."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

from current_gate_chain import require_current_gate_log
from production_preflight_config import load_config, nested_present

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "EXTERNAL_SECRET_PROVIDER_PREFLIGHT.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

REFERENCE_ENV = {
    "secret_provider_reference_present": "PMX_SECRET_PROVIDER",
    "kms_key_reference_present": "PMX_KMS_KEY_ID",
    "rotation_evidence_reference_present": "PMX_SECRET_ROTATION_EVIDENCE_ID",
    "break_glass_review_reference_present": "PMX_BREAK_GLASS_REVIEW_ID",
}
CONFIG_FIELDS = {
    "secret_provider_reference_present": ("secret_provider", "provider_ref"),
    "kms_key_reference_present": ("secret_provider", "kms_key_ref"),
    "rotation_evidence_reference_present": ("secret_provider", "rotation_evidence_ref"),
    "break_glass_review_reference_present": ("secret_provider", "break_glass_review_ref"),
}
EXPLICIT_SENSITIVE_NAMES = [
    "POLYMARKET_PRIVATE_KEY",
    "POLYMARKET_CLOB_API_SECRET",
    "CLOB_SECRET",
]
SENSITIVE_NAME_PATTERNS = [
    re.compile(r"^PMX_ACCT_[A-Z0-9_]+_(POLYMARKET_PRIVATE_KEY|CLOB_API_KEY|CLOB_SECRET|CLOB_PASS_PHRASE|CLOB_API_KEY_NONCE|CLOB_FUNDER)$"),
    re.compile(r"^POLY(_API_KEY|_API_SECRET|_API_PASSPHRASE)$"),
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def present(name: str) -> bool:
    return bool(os.environ.get(name, "").strip())


def sensitive_names() -> list[str]:
    names = set(EXPLICIT_SENSITIVE_NAMES)
    for name in os.environ:
        if any(pattern.match(name) for pattern in SENSITIVE_NAME_PATTERNS):
            names.add(name)
    return sorted(names)


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during external secret provider preflight")

    required_tokens = [
        "secret_provider_reference_present",
        "kms_key_reference_present",
        "rotation_evidence_reference_present",
        "break_glass_review_reference_present",
        "plaintext_secret_values_absent",
        "provider_health_check_required",
        "credential_rotation_required",
        "break_glass_review_required",
        "external_secret_custody_ready = false",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("external secret provider preflight document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"external secret provider preflight document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("59-external-secret-provider-preflight.log", "external secret provider preflight", failures)
    if '"external_secret_provider_preflight_validation"' not in manifest:
        failures.append("evidence manifest must include external_secret_provider_preflight_validation")
    if "59-external-secret-provider-preflight.log" not in manifest:
        failures.append("evidence manifest must capture external secret provider preflight log")

    config, config_path, config_failures = load_config()
    failures.extend(config_failures)
    signals = {
        label: present(REFERENCE_ENV[label])
        or nested_present(config, *CONFIG_FIELDS[label])
        for label in REFERENCE_ENV
    }
    sensitive_env_present = {name: present(name) for name in sensitive_names()}
    external_ready = all(signals.values())
    result = {
        "status": "fail" if failures else "pass",
        "signals": signals,
        "config_path": str(config_path.relative_to(ROOT)) if config_path and ROOT in config_path.parents else str(config_path) if config_path else None,
        "config_loaded": bool(config),
        "provider_health_check_required": True,
        "credential_rotation_required": True,
        "break_glass_review_required": True,
        "plaintext_secret_values_absent": True,
        "sensitive_env_detected_as_boolean_only": sensitive_env_present,
        "external_secret_custody_ready": external_ready,
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
