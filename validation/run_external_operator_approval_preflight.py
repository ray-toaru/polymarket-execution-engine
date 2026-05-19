#!/usr/bin/env python3
"""Validate the external operator-approval preflight contract."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log
from production_preflight_config import load_config, nested_present

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "EXTERNAL_OPERATOR_APPROVAL_PREFLIGHT.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

REFERENCE_ENV = {
    "approval_id_present": "PMX_OPERATOR_APPROVAL_ID",
    "approval_hash_present": "PMX_OPERATOR_APPROVAL_HASH",
    "approval_ticket_present": "PMX_OPERATOR_APPROVAL_TICKET",
    "approver_identity_present": "PMX_OPERATOR_APPROVER_ID",
    "approval_expiry_present": "PMX_OPERATOR_APPROVAL_EXPIRES_AT",
    "approval_scope_present": "PMX_OPERATOR_APPROVAL_SCOPE",
}
CONFIG_FIELDS = {
    "approval_id_present": ("operator_approval", "approval_id"),
    "approval_hash_present": ("operator_approval", "approval_hash"),
    "approval_ticket_present": ("operator_approval", "ticket_ref"),
    "approver_identity_present": ("operator_approval", "approver_identity_ref"),
    "approval_expiry_present": ("operator_approval", "expires_at"),
    "approval_scope_present": ("operator_approval", "scope"),
}


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def present(name: str) -> bool:
    return bool(os.environ.get(name, "").strip())


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during external operator approval preflight")

    required_tokens = [
        "approval_id_present",
        "approval_hash_present",
        "approval_ticket_present",
        "approver_identity_present",
        "approval_expiry_present",
        "approval_scope_present",
        "dual_control_required",
        "approval_replay_block_required",
        "approval_expiry_enforced",
        "operator_approval_ready = false",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("external operator approval preflight document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"external operator approval preflight document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("60-external-operator-approval-preflight.log", "external operator approval preflight", failures)
    if '"external_operator_approval_preflight_validation"' not in manifest:
        failures.append("evidence manifest must include external_operator_approval_preflight_validation")
    if "60-external-operator-approval-preflight.log" not in manifest:
        failures.append("evidence manifest must capture external operator approval preflight log")

    config, config_path, config_failures = load_config()
    failures.extend(config_failures)
    signals = {
        label: present(REFERENCE_ENV[label])
        or nested_present(config, *CONFIG_FIELDS[label])
        for label in REFERENCE_ENV
    }
    operator_ready = all(signals.values())
    result = {
        "status": "fail" if failures else "pass",
        "signals": signals,
        "config_path": str(config_path.relative_to(ROOT)) if config_path and ROOT in config_path.parents else str(config_path) if config_path else None,
        "config_loaded": bool(config),
        "dual_control_required": True,
        "approval_replay_block_required": True,
        "approval_expiry_enforced": True,
        "operator_approval_ready": operator_ready,
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
