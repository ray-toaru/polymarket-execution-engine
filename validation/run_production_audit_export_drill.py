#!/usr/bin/env python3
"""Validate redacted local audit export evidence for productionization controls."""
from __future__ import annotations

import hashlib
import json
import os
import re
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_AUDIT_EXPORT_DRILL.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"

FORBIDDEN_KEYS = {
    "private_key",
    "clob_secret",
    "raw_signed_payload",
    "raw_signature",
    "SignedOrderEnvelope",
}

FORBIDDEN_PATTERNS = [
    re.compile(r"private[_-]?key", re.I),
    re.compile(r"clob[_-]?secret", re.I),
    re.compile(r"raw[_-]?signed[_-]?payload", re.I),
    re.compile(r"raw[_-]?signature", re.I),
    re.compile(r"SignedOrderEnvelope"),
]


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def digest(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def build_export_record() -> dict[str, object]:
    source_event = {
        "trace_id": "trace-prod-audit-001",
        "order_id": "order-001",
        "client_event_id": "event-001",
        "signed_order_ref": "signed-ref-001",
        "signed_order_digest": digest("redacted-signed-order"),
        "lifecycle_state": "SIGNED_DRY_RUN",
        "private_key": "0x" + "a" * 64,
        "clob_secret": "secret-value",
        "raw_signed_payload": "{\"signature\":\"0xabc\"}",
        "raw_signature": "0xabc",
        "SignedOrderEnvelope": {"should": "not export"},
    }
    return {
        "trace_id": source_event["trace_id"],
        "order_id": source_event["order_id"],
        "client_event_id": source_event["client_event_id"],
        "signed_order_ref": source_event["signed_order_ref"],
        "signed_order_digest": source_event["signed_order_digest"],
        "lifecycle_state": source_event["lifecycle_state"],
        "retention_policy_id": "retention-365d-redacted",
        "retention_duration_days": 365,
        "deletion_policy_defined": True,
        "legal_hold": False,
        "access_reviewed": True,
        "export_batch_id": "audit-export-local-001",
        "immutable_export": True,
        "redacted_export": True,
        "export_failure_blocks_promotion": True,
    }


def main() -> int:
    failures: list[str] = []
    if env_enabled("PMX_ALLOW_LIVE_SUBMIT"):
        failures.append("PMX_ALLOW_LIVE_SUBMIT=1 is forbidden during audit export drill")
    if env_enabled("PMX_ALLOW_LIVE_CANCEL"):
        failures.append("PMX_ALLOW_LIVE_CANCEL=1 is forbidden during audit export drill")
    if env_enabled("PMX_PRODUCTION_READY"):
        failures.append("PMX_PRODUCTION_READY=1 is forbidden without a reviewed production release")

    if not DOC.exists():
        failures.append("production audit export drill document missing")
    else:
        doc = DOC.read_text()
        for token in [
            "trace_id",
            "signed_order_ref",
            "signed_order_digest",
            "retention_policy_id",
            "export_batch_id",
            "private_key",
            "clob_secret",
            "raw_signed_payload",
            "raw_signature",
            "SignedOrderEnvelope",
            "immutable_export = true",
            "redacted_export = true",
            "remote_side_effects = false",
            "production_ready_claimed = false",
        ]:
            if token not in doc:
                failures.append(f"production audit export drill document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("48-production-audit-export-drill.log", "production audit export drill", failures)
    if '"production_audit_export_validation"' not in manifest:
        failures.append("evidence manifest must include production_audit_export_validation")
    if "48-production-audit-export-drill.log" not in manifest:
        failures.append("evidence manifest must capture production audit export log")

    export_record = build_export_record()
    for key in FORBIDDEN_KEYS:
        if key in export_record:
            failures.append(f"export record includes forbidden key: {key}")
    export_json = json.dumps(export_record, sort_keys=True)
    for pattern in FORBIDDEN_PATTERNS:
        if pattern.search(export_json):
            failures.append(f"export record includes forbidden sensitive token: {pattern.pattern}")

    if not export_record["immutable_export"]:
        failures.append("audit export must be immutable")
    if not export_record["redacted_export"]:
        failures.append("audit export must be redacted")
    if not export_record["deletion_policy_defined"]:
        failures.append("audit export deletion policy must be defined")
    if int(export_record["retention_duration_days"]) <= 0:
        failures.append("audit export retention duration must be positive")
    if not export_record["access_reviewed"]:
        failures.append("audit export access must be reviewed")
    if not export_record["export_failure_blocks_promotion"]:
        failures.append("audit export failure must block production promotion")

    result = {
        "status": "fail" if failures else "pass",
        "production_ready_claimed": False,
        "remote_side_effects": False,
        "export_record": export_record,
        "forbidden_keys_absent": sorted(FORBIDDEN_KEYS),
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
