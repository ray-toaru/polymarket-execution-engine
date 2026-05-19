#!/usr/bin/env python3
"""Validate production preflight config shape and forbidden sensitive material."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log
from production_preflight_config import (
    DEFAULT_CONFIG,
    load_config,
    nested_present,
)

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_GUARD.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

REQUIRED_FIELDS = {
    "secret_provider_reference_present": ("secret_provider", "provider_ref"),
    "kms_key_reference_present": ("secret_provider", "kms_key_ref"),
    "rotation_evidence_reference_present": ("secret_provider", "rotation_evidence_ref"),
    "break_glass_review_reference_present": ("secret_provider", "break_glass_review_ref"),
    "approval_id_present": ("operator_approval", "approval_id"),
    "approval_hash_present": ("operator_approval", "approval_hash"),
    "approval_ticket_present": ("operator_approval", "ticket_ref"),
    "approver_identity_present": ("operator_approval", "approver_identity_ref"),
    "approval_expiry_present": ("operator_approval", "expires_at"),
    "approval_scope_present": ("operator_approval", "scope"),
    "alert_provider_reference_present": ("alert_routing", "provider_ref"),
    "alert_route_reference_present": ("alert_routing", "route_ref"),
    "pager_escalation_policy_present": ("alert_routing", "pager_escalation_policy_ref"),
    "dashboard_reference_present": ("alert_routing", "dashboard_ref"),
    "alert_test_evidence_present": ("alert_routing", "alert_test_evidence_ref"),
}


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def relative(path: Path | None) -> str | None:
    if path is None:
        return None
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during production preflight config guard")

    required_tokens = [
        "production_preflight_config_schema_version = 1",
        "forbidden_sensitive_keys_absent = true",
        "forbidden_sensitive_values_absent = true",
        "references_only_no_secret_values = true",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ] + list(REQUIRED_FIELDS)
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production preflight config guard document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"production preflight config guard document missing token: {token}")

    if not DEFAULT_CONFIG.exists():
        failures.append("production preflight example config missing")

    manifest = MANIFEST.read_text()
    require_current_gate_log("62-production-preflight-config-guard.log", "production preflight config guard", failures)
    if '"production_preflight_config_validation"' not in manifest:
        failures.append("evidence manifest must include production_preflight_config_validation")
    if "62-production-preflight-config-guard.log" not in manifest:
        failures.append("evidence manifest must capture production preflight config guard log")

    config, config_path, config_failures = load_config(use_default=True)
    failures.extend(config_failures)
    field_signals = {
        label: nested_present(config, section, field)
        for label, (section, field) in REQUIRED_FIELDS.items()
    }
    if config and not all(field_signals.values()):
        missing = [label for label, value in field_signals.items() if not value]
        failures.append("production preflight config missing required references: " + ", ".join(missing))

    result = {
        "status": "fail" if failures else "pass",
        "config_path": relative(config_path),
        "config_loaded": bool(config),
        "production_preflight_config_schema_version": config.get("schema_version") if config else None,
        "field_signals": field_signals,
        "forbidden_sensitive_keys_absent": not any(
            failure.startswith("forbidden sensitive config key") for failure in config_failures
        ),
        "forbidden_sensitive_values_absent": not any(
            failure.startswith("forbidden sensitive-looking config value") for failure in config_failures
        ),
        "references_only_no_secret_values": not config_failures,
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
