#!/usr/bin/env python3
"""Guard trace, redaction, and evidence controls for non-production observability."""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "crates" / "pmx-api" / "src" / "lib.rs"
SERVICE = ROOT / "crates" / "pmx-service" / "src" / "lib.rs"
STORE = ROOT / "crates" / "pmx-store" / "src" / "lib.rs"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
MIGRATION = ROOT / "migrations" / "0003_order_event_trace.sql"
SHADOW = ROOT / "validation" / "run_shadow_execution_drill.py"
RECONCILE = ROOT / "validation" / "run_reconciliation_drift_drill.py"
ROLLBACK = ROOT / "validation" / "run_kill_switch_rollback_drill.py"
GATES = ROOT / "validation" / "run_v0_24_gates.sh"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DOCS = [
    ROOT / "docs" / "PRODUCTION_HARDENING_SPEC.md",
    ROOT / "docs" / "PRODUCTION_EVIDENCE_CONTROLS.md",
    ROOT / "docs" / "PRODUCTION_CONTROLS_MATRIX.md",
]

REQUIRED = {
    API: [
        "correlation_id_from_headers",
        "api_error_with_correlation",
        "redacted_payload_envelope",
        "x-correlation-id",
    ],
    SERVICE: [
        "correlation_id: correlation_id.clone()",
        "redacted sign-only ref",
        "record_non_live_reconcile_observation",
    ],
    STORE: [
        "pub correlation_id: Option<String>",
        "list_order_lifecycle_events",
        "list_admin_audit_events",
    ],
    POSTGRES: [
        "INSERT INTO order_events",
        "correlation_id",
    ],
    MIGRATION: [
        "ADD COLUMN IF NOT EXISTS correlation_id",
        "idx_order_events_order_correlation",
    ],
    SHADOW: [
        "trace_id",
        "credentials_logged",
        "raw_signed_payload_logged",
        "raw_signature_logged",
    ],
    RECONCILE: [
        "trace_id",
        "operator_required",
        "remote_side_effects",
    ],
    ROLLBACK: [
        "fallback_mode",
        "operator_required",
        "remote_side_effects",
    ],
}

FORBIDDEN_PUBLIC_TOKENS = [
    re.compile(r"private[_-]?key", re.I),
    re.compile(r"clob[_-]?secret", re.I),
    re.compile(r"raw[_-]?signed[_-]?payload", re.I),
    re.compile(r"raw[_-]?signature", re.I),
]


def main() -> int:
    failures: list[str] = []
    for path, tokens in REQUIRED.items():
        text = path.read_text()
        for token in tokens:
            if token not in text:
                failures.append(f"{path.relative_to(ROOT)} missing {token}")

    for doc in DOCS:
        text = doc.read_text().lower()
        for token in ["per-order trace", "redacted", "audit", "evidence"]:
            if token not in text:
                failures.append(f"{doc.relative_to(ROOT)} missing observability token: {token}")

    gates = GATES.read_text()
    manifest = MANIFEST.read_text()
    if "43-observability-evidence.log" not in gates:
        failures.append("run_v0_24_gates.sh must emit observability evidence log")
    if '"observability_evidence_validation"' not in manifest:
        failures.append("evidence manifest must include observability_evidence_validation")
    if "43-observability-evidence.log" not in manifest:
        failures.append("evidence manifest must capture observability evidence log")

    public_contract_text = (ROOT / "openapi" / "executor.v1.yaml").read_text()
    for pattern in FORBIDDEN_PUBLIC_TOKENS:
        if pattern.search(public_contract_text):
            failures.append(f"OpenAPI contains forbidden public sensitive token: {pattern.pattern}")

    result = {
        "status": "fail" if failures else "pass",
        "remote_side_effects": False,
        "checks": [
            "correlation_id",
            "redacted_payload_envelope",
            "order_event_trace",
            "admin_audit_query",
            "shadow_trace_id",
            "reconcile_trace_id",
            "rollback_fallback_mode",
        ],
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
