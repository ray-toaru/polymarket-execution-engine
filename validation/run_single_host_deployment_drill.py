#!/usr/bin/env python3
"""Validate the single-host limited deployment templates remain fail-closed."""
from __future__ import annotations

import json
import os
import socket
import subprocess
import time
from http.client import HTTPConnection
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DEPLOY = ROOT / "deploy" / "single-host"
README = DEPLOY / "README.md"
API_ENV = DEPLOY / "env" / "pmx-api.env.example"
CANARY_ENV = DEPLOY / "env" / "pmx-real-funds-canary.env.example"
API_SERVICE = DEPLOY / "systemd" / "pmx-api.service"
CANARY_SERVICE = DEPLOY / "systemd" / "pmx-real-funds-canary@.service"
PREFLIGHT = DEPLOY / "bin" / "pmx-single-host-preflight.sh"
ROLLBACK = DEPLOY / "bin" / "pmx-single-host-rollback.sh"
CANARY_PACKAGE_PREFLIGHT = DEPLOY / "bin" / "pmx-single-host-canary-package-preflight.sh"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
REQUIRED_FILES = [
    README,
    API_ENV,
    CANARY_ENV,
    API_SERVICE,
    CANARY_SERVICE,
    PREFLIGHT,
    ROLLBACK,
    CANARY_PACKAGE_PREFLIGHT,
]
FAIL_CLOSED_FLAGS = [
    "PMX_LIVE_SUBMIT_ENABLED=0",
    "PMX_LIVE_CANCEL_ENABLED=0",
    "PMX_PRODUCTION_DEPLOYMENT_ENABLED=0",
    "PMX_ALLOW_LIVE_SUBMIT=0",
    "PMX_ALLOW_LIVE_CANCEL=0",
    "PMX_ALLOW_REAL_FUNDS_CANARY=0",
]
FORBIDDEN_VALUE_FRAGMENTS = [
    "-----BEGIN",
    "PRIVATE KEY-----",
    "clob_secret=",
    "raw_signature=",
    "raw_signed_payload=",
    "signed_order_envelope=",
    "PMX_ALLOW_LIVE_SUBMIT=1",
    "PMX_ALLOW_LIVE_CANCEL=1",
    "PMX_ALLOW_REAL_FUNDS_CANARY=1",
    "PMX_PRODUCTION_DEPLOYMENT_ENABLED=1",
]


def free_local_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def run_api_bind_smoke(failures: list[str]) -> bool:
    port = free_local_port()
    env = os.environ.copy()
    env.update(
        {
            "PMX_API_BIND_ADDR": f"127.0.0.1:{port}",
            "PMX_API_PROFILE": "local",
            "PMX_API_STORAGE": "in_memory_scaffold",
            "PMX_API_SERVICE_TOKEN": "single-host-smoke-service-token",
            "PMX_API_ADMIN_TOKEN": "single-host-smoke-admin-token",
            "PMX_ALLOW_LIVE_SUBMIT": "0",
            "PMX_ALLOW_LIVE_CANCEL": "0",
            "PMX_ALLOW_REAL_FUNDS_CANARY": "0",
        }
    )
    proc = subprocess.Popen(
        ["cargo", "run", "-p", "pmx-api", "--locked"],
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        deadline = time.monotonic() + 35
        last_error = ""
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                stderr = (proc.stderr.read() if proc.stderr else "")[-2000:]
                failures.append(f"pmx-api bind smoke exited early code={proc.returncode}: {stderr}")
                return False
            try:
                conn = HTTPConnection("127.0.0.1", port, timeout=1)
                conn.request(
                    "GET",
                    "/v1/health",
                    headers={"Authorization": "Bearer single-host-smoke-service-token"},
                )
                response = conn.getresponse()
                body = response.read().decode("utf-8", errors="replace")
                conn.close()
                if response.status == 200 and "in_memory_scaffold" in body:
                    return True
                last_error = f"status={response.status} body={body[:200]}"
            except OSError as exc:
                last_error = str(exc)
            time.sleep(0.5)
        failures.append(f"pmx-api bind smoke did not become healthy: {last_error}")
        return False
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=5)


def read(path: Path) -> str:
    return path.read_text()


def main() -> int:
    failures: list[str] = []
    require_current_gate_log(
        "69-single-host-deployment-drill.log",
        "single-host deployment drill",
        failures,
    )

    for path in REQUIRED_FILES:
        if not path.exists():
            failures.append(f"missing deployment template: {path.relative_to(ROOT)}")

    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    readme = read(README)
    api_env = read(API_ENV)
    canary_env = read(CANARY_ENV)
    api_service = read(API_SERVICE)
    canary_service = read(CANARY_SERVICE)
    preflight = read(PREFLIGHT)
    rollback = read(ROLLBACK)
    candidate_preflight = read(CANARY_PACKAGE_PREFLIGHT)
    combined_templates = "\n".join([api_env, canary_env, api_service, canary_service, preflight, rollback])

    for token in [
        "single-host limited deployment",
        "not production-ready evidence",
        "long-running HTTP listener",
        "non-live API smoke",
        "PMX_LIVE_SUBMIT_ENABLED=0",
        "PMX_ALLOW_REAL_FUNDS_CANARY=0",
        "pass://polymarket-execution-engine/controlled-canary",
        "runs `pmx-real-funds-canary` in `--dry-run` mode",
        "reviewed `go` release decision",
    ]:
        if token not in readme:
            failures.append(f"single-host README missing token: {token}")

    for label, text in [
        ("api env", api_env),
        ("canary env", canary_env),
        ("api service", api_service),
        ("canary service", canary_service),
    ]:
        for flag in FAIL_CLOSED_FLAGS:
            if flag not in text:
                failures.append(f"{label} missing fail-closed flag {flag}")

    if "Description=Polymarket execution API (single-host limited; non-live)" not in api_service:
        failures.append("api systemd unit must identify itself as a non-live API listener")
    if "Type=simple" not in api_service:
        failures.append("api systemd unit must be a long-running simple service")
    if "Restart=on-failure" not in api_service:
        failures.append("api systemd unit must restart the API listener on failure")
    if "ExecStart=/opt/polymarket-execution-engine/bin/pmx-api" not in api_service:
        failures.append("api systemd unit must run the pmx-api binary")
    if "ExecStart=/opt/polymarket-execution-engine/bin/pmx-real-funds-canary" not in canary_service:
        failures.append("canary systemd unit must start pmx-real-funds-canary binary")
    if "--dry-run" not in canary_service:
        failures.append("canary systemd unit must run dry-run mode")
    if "PMX_CANARY_MARKET_FILE" not in canary_env:
        failures.append("canary env must require external candidate market file")
    if "--market-file ${PMX_CANARY_MARKET_FILE}" not in canary_service:
        failures.append("canary systemd unit must pass external candidate market file")
    for forbidden in ["--armed", "--allow-live-submit-config", "--allow-real-funds-canary-config"]:
        if forbidden in canary_service:
            failures.append(f"canary systemd unit must not include {forbidden}")

    for guard in [
        "validation/check_live_submit_guard.py",
        "validation/check_production_readiness_guard.py",
        "validation/check_docs_evidence_governance.py",
    ]:
        if guard not in preflight:
            failures.append(f"single-host preflight missing guard {guard}")
    for token in [
        "PMX_API_SERVICE_TOKEN",
        "PMX_API_ADMIN_TOKEN",
        "PMX_API_STORAGE must be postgres",
    ]:
        if token not in preflight:
            failures.append(f"single-host preflight missing runtime config check: {token}")
    for legacy_token in [
        "PM_EXEC_SERVICE_TOKEN",
        "PM_EXEC_ADMIN_TOKEN",
        "PMX_SERVICE_TOKEN",
        "PMX_ADMIN_TOKEN",
    ]:
        if legacy_token in "\n".join([api_env, api_service, preflight]):
            failures.append(
                f"single-host API deployment must not use legacy/non-canonical auth env {legacy_token}"
            )
    for token in [
        'PMX_LIVE_SUBMIT_ENABLED:-0}" == "1"',
        'PMX_ALLOW_LIVE_SUBMIT:-0}" == "1"',
        'PMX_LIVE_CANCEL_ENABLED:-0}" == "1"',
        'PMX_PRODUCTION_DEPLOYMENT_ENABLED:-0}" == "1"',
    ]:
        if token not in preflight:
            failures.append(f"single-host preflight missing fail-closed refusal token: {token}")

    for flag in FAIL_CLOSED_FLAGS:
        if f"export {flag}" not in rollback:
            failures.append(f"single-host rollback must force {flag}")
    if "PMX_KILL_SWITCH_OPEN=0" not in rollback:
        failures.append("single-host rollback must close kill switch")

    for token in [
        "validate_controlled_canary_external_references.py",
        "single-host canary package preflight only accepts no_go release decisions",
        "release decision must keep",
        "external references must be reference-only",
        "single-host canary package preflight passed",
    ]:
        if token not in candidate_preflight:
            failures.append(f"single-host canary package preflight missing token: {token}")

    for fragment in FORBIDDEN_VALUE_FRAGMENTS:
        if fragment in combined_templates:
            failures.append(f"single-host deployment template contains forbidden live/sensitive fragment: {fragment}")

    writer = read(MANIFEST_WRITER)
    if '"single_host_deployment_validation"' not in writer:
        failures.append("evidence manifest must include single_host_deployment_validation")
    if "69-single-host-deployment-drill.log" not in writer:
        failures.append("evidence manifest must capture single-host deployment log")
    if '"single_host_canary_candidate_validation"' not in writer:
        failures.append("evidence manifest must include single_host_canary_candidate_validation")
    if "70-single-host-canary-candidate-drill.log" not in writer:
        failures.append("evidence manifest must capture single-host canary candidate log")

    api_bind_smoke = False
    if not failures:
        api_bind_smoke = run_api_bind_smoke(failures)

    result = {
        "status": "fail" if failures else "pass",
        "deployment_profile": "single-host-limited",
        "api_service_present": API_SERVICE.exists(),
        "api_bind_smoke": api_bind_smoke,
        "canary_runner_mode": "dry-run",
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
