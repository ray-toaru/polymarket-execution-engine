#!/usr/bin/env python3
"""Validate local dependency pin and SDK breakage response evidence."""
from __future__ import annotations

import json
import os
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_DEPENDENCY_BREAKAGE_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
ADAPTER_TOML = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.toml"
ADAPTER_LOCK = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.lock"
SPIKE_TOML = ROOT / "adapters" / "pmx-official-sdk-spike" / "Cargo.toml"
SPIKE_LOCK = ROOT / "adapters" / "pmx-official-sdk-spike" / "Cargo.lock"
SPIKE_LIB = ROOT / "adapters" / "pmx-official-sdk-spike" / "src" / "lib.rs"

SDK_NAME = "polymarket_client_sdk_v2"
SDK_PIN = "=0.6.0-canary.1"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def contains(path: Path, token: str) -> bool:
    return token in path.read_text()


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during dependency breakage drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during dependency breakage drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production dependency breakage drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "exact_sdk_pin",
            "adapter_lockfile_present",
            "spike_lockfile_present",
            "sdk_typecheck_evidence",
            "sign_only_regression_evidence",
            "authenticated_non_trading_evidence",
            "rollback_plan",
            "compatibility_review_required",
            "freeze_live_submit",
            "downgrade_to_sign_only",
            "downgrade_to_read_only",
            "preserve_evidence",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production dependency breakage document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log(
        "49-production-dependency-breakage-drill.log",
        "production dependency breakage drill",
        failures,
    )
    if '"production_dependency_breakage_validation"' not in manifest:
        failures.append("evidence manifest must include production_dependency_breakage_validation")
    if "49-production-dependency-breakage-drill.log" not in manifest:
        failures.append("evidence manifest must capture production dependency breakage log")

    for path in [ADAPTER_TOML, SPIKE_TOML]:
        if not contains(path, SDK_NAME):
            failures.append(f"{path.relative_to(ROOT)} missing {SDK_NAME}")
        if not contains(path, f'version = "{SDK_PIN}"'):
            failures.append(f"{path.relative_to(ROOT)} missing exact SDK pin {SDK_PIN}")

    for path in [ADAPTER_LOCK, SPIKE_LOCK]:
        if not path.exists():
            failures.append(f"{path.relative_to(ROOT)} missing")
        elif not contains(path, SDK_NAME):
            failures.append(f"{path.relative_to(ROOT)} missing locked SDK package")

    for token in [SDK_NAME, SDK_PIN]:
        if not contains(SPIKE_LIB, token):
            failures.append(f"{SPIKE_LIB.relative_to(ROOT)} missing SDK evidence token {token}")

    for log_name in [
        "07-sdk-spike-typecheck.log",
        "11-sdk-adapter-test.log",
        "12-sdk-adapter-typecheck.log",
        "16-authenticated-smoke.log",
        "17-sign-only-dry-run.log",
        "37-sdk-regression-suite-guard.log",
        "35-sdk-standard-sign-only-guard.log",
    ]:
        require_current_gate_log(log_name, f"dependency breakage source evidence {log_name}", failures)

    response = {
        "exact_sdk_pin": SDK_PIN,
        "adapter_lockfile_present": ADAPTER_LOCK.exists(),
        "spike_lockfile_present": SPIKE_LOCK.exists(),
        "sdk_typecheck_evidence": True,
        "sign_only_regression_evidence": True,
        "authenticated_non_trading_evidence": True,
        "rollback_plan": "revert SDK pin and lockfiles, keep live submit frozen, rerun current gates",
        "compatibility_review_required": True,
        "freeze_live_submit": True,
        "downgrade_to_sign_only": True,
        "downgrade_to_read_only": True,
        "preserve_evidence": True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "fallback_mode": "sign-only",
        "remote_side_effects": False,
        "production_ready_claimed": False,
    }

    result = {
        "status": "fail" if failures else "pass",
        "dependency_breakage_response": response,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
