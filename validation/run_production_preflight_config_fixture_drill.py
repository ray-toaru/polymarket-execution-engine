#!/usr/bin/env python3
"""Validate positive and negative production preflight config fixtures."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from current_gate_chain import require_current_gate_log
from production_preflight_config import load_config

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_FIXTURE_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
POSITIVE_FIXTURE = ROOT / "config" / "production-preflight.fixture.json"
NEGATIVE_FIXTURE = ROOT / "config" / "production-preflight.invalid-sensitive.fixture.json"
FORBIDDEN_FIXTURE_VALUE = "fixture-sensitive-value-must-not-be-logged"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def run_script(script: str, config_path: Path) -> tuple[int, dict[str, object], str]:
    env = os.environ.copy()
    env["PMX_PRODUCTION_PREFLIGHT_CONFIG"] = str(config_path.relative_to(ROOT))
    completed = subprocess.run(
        [sys.executable, f"validation/{script}"],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    output = completed.stdout
    try:
        data = json.loads(output)
    except json.JSONDecodeError:
        data = {}
    return completed.returncode, data, output


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during production preflight config fixture drill")

    required_tokens = [
        "fixture_secret_provider_ready = true",
        "fixture_operator_approval_ready = true",
        "fixture_alerting_ready = true",
        "fixture_live_submit_allowed = false",
        "fixture_live_cancel_allowed = false",
        "fixture_remote_side_effects = false",
        "invalid_sensitive_fixture_rejected = true",
        "invalid_sensitive_fixture_secret_value_logged = false",
        "invalid_sensitive_fixture_reports_path_only = true",
        "forbidden_sensitive_keys_absent = false",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production preflight config fixture drill document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"production preflight config fixture drill document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("63-production-preflight-config-fixture-drill.log", "production preflight config fixture drill", failures)
    if '"production_preflight_config_fixture_validation"' not in manifest:
        failures.append("evidence manifest must include production_preflight_config_fixture_validation")
    if "63-production-preflight-config-fixture-drill.log" not in manifest:
        failures.append("evidence manifest must capture production preflight config fixture drill log")

    if not POSITIVE_FIXTURE.exists():
        failures.append("positive production preflight fixture missing")
    if not NEGATIVE_FIXTURE.exists():
        failures.append("negative sensitive production preflight fixture missing")

    secret_rc, secret_data, secret_output = run_script("run_external_secret_provider_preflight.py", POSITIVE_FIXTURE)
    approval_rc, approval_data, approval_output = run_script("run_external_operator_approval_preflight.py", POSITIVE_FIXTURE)
    alert_rc, alert_data, alert_output = run_script("run_external_alert_routing_preflight.py", POSITIVE_FIXTURE)
    for name, rc in [
        ("secret provider fixture preflight", secret_rc),
        ("operator approval fixture preflight", approval_rc),
        ("alert routing fixture preflight", alert_rc),
    ]:
        if rc != 0:
            failures.append(f"{name} failed")
    fixture_outputs = secret_output + approval_output + alert_output
    if FORBIDDEN_FIXTURE_VALUE in fixture_outputs:
        failures.append("positive fixture run leaked forbidden fixture value")

    previous_config = os.environ.get("PMX_PRODUCTION_PREFLIGHT_CONFIG")
    os.environ["PMX_PRODUCTION_PREFLIGHT_CONFIG"] = str(NEGATIVE_FIXTURE.relative_to(ROOT))
    try:
        _, _, negative_failures = load_config(use_default=False)
    finally:
        if previous_config is None:
            os.environ.pop("PMX_PRODUCTION_PREFLIGHT_CONFIG", None)
        else:
            os.environ["PMX_PRODUCTION_PREFLIGHT_CONFIG"] = previous_config

    invalid_rejected = any("secret_provider.private_key" in failure for failure in negative_failures)
    invalid_value_logged = any(FORBIDDEN_FIXTURE_VALUE in failure for failure in negative_failures)
    if not invalid_rejected:
        failures.append("negative sensitive fixture was not rejected by field path")
    if invalid_value_logged:
        failures.append("negative sensitive fixture failure leaked fixture value")

    result = {
        "status": "fail" if failures else "pass",
        "positive_fixture": str(POSITIVE_FIXTURE.relative_to(ROOT)),
        "negative_fixture": str(NEGATIVE_FIXTURE.relative_to(ROOT)),
        "fixture_secret_provider_ready": bool(secret_data.get("external_secret_custody_ready")),
        "fixture_operator_approval_ready": bool(approval_data.get("operator_approval_ready")),
        "fixture_alerting_ready": bool(alert_data.get("alerting_ready")),
        "fixture_live_submit_allowed": bool(
            secret_data.get("live_submit_allowed")
            or approval_data.get("live_submit_allowed")
            or alert_data.get("live_submit_allowed")
        ),
        "fixture_live_cancel_allowed": bool(
            secret_data.get("live_cancel_allowed")
            or approval_data.get("live_cancel_allowed")
            or alert_data.get("live_cancel_allowed")
        ),
        "fixture_remote_side_effects": bool(
            secret_data.get("remote_side_effects")
            or approval_data.get("remote_side_effects")
            or alert_data.get("remote_side_effects")
        ),
        "invalid_sensitive_fixture_rejected": invalid_rejected,
        "invalid_sensitive_fixture_secret_value_logged": invalid_value_logged,
        "invalid_sensitive_fixture_reports_path_only": invalid_rejected and not invalid_value_logged,
        "forbidden_sensitive_keys_absent": False,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    if not result["fixture_secret_provider_ready"]:
        failures.append("positive fixture did not make secret provider ready")
    if not result["fixture_operator_approval_ready"]:
        failures.append("positive fixture did not make operator approval ready")
    if not result["fixture_alerting_ready"]:
        failures.append("positive fixture did not make alerting ready")
    if result["fixture_live_submit_allowed"]:
        failures.append("positive fixture must not allow live submit")
    if result["fixture_live_cancel_allowed"]:
        failures.append("positive fixture must not allow live cancel")
    if result["fixture_remote_side_effects"]:
        failures.append("positive fixture must not cause remote side effects")
    result["status"] = "fail" if failures else "pass"
    result["failures"] = failures
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
