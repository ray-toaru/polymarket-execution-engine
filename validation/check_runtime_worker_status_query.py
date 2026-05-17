#!/usr/bin/env python3
"""Guard the runtime worker status query and its evidence wiring."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "crates" / "pmx-api" / "src" / "lib.rs"
API_FAKE_E2E = ROOT / "crates" / "pmx-api" / "tests" / "http_and_fake_e2e.rs"
API_PG_E2E = ROOT / "crates" / "pmx-api" / "tests" / "http_postgres_e2e.rs"
OPENAPI = ROOT / "openapi" / "executor.v1.yaml"
STORE = ROOT / "crates" / "pmx-store" / "src" / "lib.rs"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
SERVICE = ROOT / "crates" / "pmx-service" / "src" / "lib.rs"
GATES = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST = ROOT / "validation" / "write_v0_23_evidence_manifest.py"
TEMPLATE = ROOT / "validation" / "templates" / "evidence_manifest.template.json"
DOC = ROOT / "docs" / "RUNTIME_WORKER_MODEL.md"

REQUIRED = {
    API: [
        '/v1/runtime/workers',
        'Operation::ReadReport',
        'RuntimeWorkerStatusListQuery',
        'RuntimeWorkerStatusReport',
        'list_runtime_worker_status',
    ],
    OPENAPI: [
        '/v1/runtime/workers',
        'listRuntimeWorkerStatus',
        'RuntimeWorkerStatusReport',
        'RuntimeWorkerHeartbeat',
        'RuntimeWorkerObservation',
        'additionalProperties: false',
    ],
    STORE: [
        'pub struct RuntimeWorkerStatusQuery',
        'pub struct RuntimeWorkerStatusReport',
        'pub trait RuntimeWorkerStatusStore',
        'in_memory_lists_runtime_worker_status',
    ],
    POSTGRES: [
        'impl RuntimeWorkerStatusStore for PostgresStore',
        'FROM worker_health',
        'FROM runtime_worker_observations',
        'postgres_lists_runtime_worker_status',
    ],
    SERVICE: [
        'RuntimeWorkerStatusStore',
        'account_id must be non-empty',
        'service_lists_runtime_worker_status',
    ],
    API_FAKE_E2E: [
        '/v1/runtime/workers?account_id=acct-http-e2e-1&limit=20',
        'runtime workers',
    ],
    API_PG_E2E: [
        '/v1/runtime/workers?account_id={account_id}&limit=20',
        'runtime worker status response',
    ],
    GATES: [
        '42-runtime-worker-status-query.log',
        'check_runtime_worker_status_query.py',
    ],
    MANIFEST: [
        '"runtime_worker_status_validation"',
        '42-runtime-worker-status-query.log',
    ],
    TEMPLATE: [
        '"runtime_worker_status_validation"',
        '42-runtime-worker-status-query.log',
    ],
    DOC: [
        'RuntimeWorkerStatusStore',
        '/v1/runtime/workers',
        'no trading side effect',
    ],
}


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def main() -> int:
    failures: list[str] = []
    for path, needles in REQUIRED.items():
        if not path.exists():
            failures.append(f"missing artifact: {path.relative_to(ROOT)}")
            continue
        text = path.read_text()
        for needle in needles:
            if needle not in text:
                failures.append(f"{path.relative_to(ROOT)} missing {needle}")
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is not allowed during runtime status query guard")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is not allowed during runtime status query guard")

    result = {
        "status": "fail" if failures else "pass",
        "route": "/v1/runtime/workers",
        "live_submit_env_enabled": env_enabled("PMX_ALLOW_LIVE_SUBMIT"),
        "live_cancel_env_enabled": env_enabled("PMX_ALLOW_LIVE_CANCEL"),
        "remote_trading_side_effect": "not_executed",
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
