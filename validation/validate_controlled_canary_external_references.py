#!/usr/bin/env python3
"""Validate controlled-canary external references are complete and reference-only."""
from __future__ import annotations

import json
import argparse
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config"
TEMPLATE = CONFIG / "controlled-canary.external-references.template.json"
EXAMPLE = CONFIG / "controlled-canary.external-references.example.json"
INVALID_SENSITIVE = CONFIG / "controlled-canary.external-references.invalid-sensitive.fixture.json"

EXPECTED_ARTIFACT_SHA256 = "c0c22c91541d48c508a588b06a2fa5d7051bc6c8e29df626de67a59cc96c24e6"
EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256 = "a67cff633141e1c619b4d422cbc6e09e427d004d9580996c4f00e31d7bebcafd"
EXPECTED_RUN_IDS = {
    "root_ci_run_id": "26254755001",
    "hermes_ci_run_id": "26198048337",
    "execution_engine_ci_run_id": "26254745573",
    "credentialed_sdk_run_id": "local-current-gates-20260521",
}
REQUIRED_FIELDS = {
    "github_evidence": [
        "root_ci_run_id",
        "hermes_ci_run_id",
        "execution_engine_ci_run_id",
        "credentialed_sdk_run_id",
    ],
    "secret_custody": [
        "provider_ref",
        "kms_key_ref",
        "rotation_evidence_ref",
        "break_glass_review_ref",
    ],
    "operator_approval": [
        "approval_id",
        "approval_hash",
        "ticket_ref",
        "approver_identity_ref",
        "expires_at",
        "scope",
    ],
    "alert_routing": [
        "provider_ref",
        "route_ref",
        "pager_escalation_policy_ref",
        "dashboard_ref",
        "alert_test_evidence_ref",
    ],
    "runbooks": [
        "rollback_runbook_ref",
        "incident_runbook_ref",
        "canary_retry_policy_ref",
    ],
}
FORBIDDEN_KEYS = {
    "private_key",
    "privateKey",
    "clob_secret",
    "clobSecret",
    "api_secret",
    "apiSecret",
    "raw_signature",
    "rawSignature",
    "raw_signed_payload",
    "rawSignedPayload",
    "signed_order_envelope",
    "SignedOrderEnvelope",
}
FORBIDDEN_VALUE_FRAGMENTS = (
    "-----BEGIN",
    "PRIVATE KEY",
    "fixture-sensitive-value-must-not-be-logged",
    "clob_secret=",
    "raw_signature=",
    "raw_signed_payload=",
)
AUTHORIZATION_FLAGS = [
    "live_submit_allowed",
    "live_cancel_allowed",
    "real_funds_canary_authorized",
    "remote_side_effects",
    "production_ready_claimed",
]


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def has_placeholder(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith("REPLACE_WITH_")
    if isinstance(value, dict):
        return any(has_placeholder(child) for child in value.values())
    if isinstance(value, list):
        return any(has_placeholder(child) for child in value)
    return False


def placeholder_paths(value: object, path: str = "") -> list[str]:
    paths: list[str] = []
    if isinstance(value, str):
        if has_placeholder(value):
            paths.append(path)
    elif isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}" if path else str(key)
            paths.extend(placeholder_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            paths.extend(placeholder_paths(child, f"{path}[{index}]"))
    return paths


def is_sha256(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(ch in "0123456789abcdefABCDEF" for ch in value)


def validate_no_sensitive_material(data: object) -> list[str]:
    failures: list[str] = []

    def walk(value: object, path: str) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                child_path = f"{path}.{key}" if path else str(key)
                if key in FORBIDDEN_KEYS:
                    failures.append(f"forbidden sensitive reference key: {child_path}")
                walk(child, child_path)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                walk(child, f"{path}[{index}]")
        elif isinstance(value, str):
            if any(fragment in value for fragment in FORBIDDEN_VALUE_FRAGMENTS):
                failures.append(f"forbidden sensitive-looking reference value: {path}")

    walk(data, "")
    return failures


def validate_shape(data: dict[str, Any], label: str, *, allow_placeholders: bool) -> list[str]:
    failures: list[str] = []
    if data.get("schema_version") != 1:
        failures.append(f"{label}: schema_version must be 1")
    if data.get("source_release") != "v0.25.0":
        failures.append(f"{label}: source_release must be v0.25.0")
    if data.get("references_only_no_secret_values") is not True:
        failures.append(f"{label}: references_only_no_secret_values must be true")
    for flag in AUTHORIZATION_FLAGS:
        if data.get(flag) is not False:
            failures.append(f"{label}: {flag} must be false")
    for section, fields in REQUIRED_FIELDS.items():
        block = data.get(section)
        if not isinstance(block, dict):
            failures.append(f"{label}: missing section {section}")
            continue
        for field in fields:
            value = block.get(field)
            if not isinstance(value, str) or not value.strip():
                failures.append(f"{label}: missing {section}.{field}")
            elif not allow_placeholders and has_placeholder(value):
                failures.append(f"{label}: unresolved placeholder {section}.{field}")
    failures.extend(f"{label}: {failure}" for failure in validate_no_sensitive_material(data))
    if not allow_placeholders:
        if not is_sha256(data.get("artifact_sha256")):
            failures.append(f"{label}: artifact_sha256 must be 64-hex")
        if not is_sha256(data.get("evidence_manifest_sha256")):
            failures.append(f"{label}: evidence_manifest_sha256 must be 64-hex")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--file",
        type=Path,
        help="Validate an operator-supplied external reference candidate instead of built-in fixtures.",
    )
    parser.add_argument(
        "--allow-placeholders",
        action="store_true",
        help="Allow REPLACE_WITH_* placeholders for generated local review material.",
    )
    args = parser.parse_args()

    if args.file:
        candidate = load(args.file)
        failures = validate_shape(candidate, str(args.file), allow_placeholders=args.allow_placeholders)
        placeholders = placeholder_paths(candidate)
        result = {
            "status": "fail" if failures else "pass",
            "file": str(args.file),
            "allow_placeholders": args.allow_placeholders,
            "placeholders_remaining": placeholders,
            "references_only_no_secret_values": candidate.get("references_only_no_secret_values") is True,
            "live_submit_allowed": False,
            "live_cancel_allowed": False,
            "real_funds_canary_authorized": False,
            "remote_side_effects": False,
            "production_ready_claimed": False,
            "failures": failures,
        }
        print(json.dumps(result, indent=2, sort_keys=True))
        return 1 if failures else 0

    failures: list[str] = []
    for path in [TEMPLATE, EXAMPLE, INVALID_SENSITIVE]:
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")
    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    template = load(TEMPLATE)
    example = load(EXAMPLE)
    invalid_sensitive = load(INVALID_SENSITIVE)

    failures.extend(validate_shape(template, "template", allow_placeholders=True))
    if not has_placeholder(template.get("artifact_sha256")) or not has_placeholder(template.get("evidence_manifest_sha256")):
        failures.append("template must keep artifact/evidence hashes as placeholders")

    failures.extend(validate_shape(example, "example", allow_placeholders=False))
    if example.get("artifact_sha256") != EXPECTED_ARTIFACT_SHA256:
        failures.append("example must bind v0.25.0 artifact SHA-256")
    if example.get("evidence_manifest_sha256") != EXPECTED_REVIEWED_EXAMPLE_MANIFEST_SHA256:
        failures.append("example must bind reviewed v0.25.0 example evidence manifest SHA-256")
    for key, expected in EXPECTED_RUN_IDS.items():
        if example.get("github_evidence", {}).get(key) != expected:
            failures.append(f"example must bind GitHub evidence {key}")

    invalid_failures = validate_shape(invalid_sensitive, "invalid_sensitive", allow_placeholders=False)
    if not invalid_failures:
        failures.append("invalid sensitive fixture must be rejected")
    invalid_text = "\n".join(invalid_failures)
    if "forbidden sensitive reference key: secret_custody.private_key" not in invalid_text:
        failures.append("invalid sensitive fixture must reject the sensitive key by field path")
    if "fixture-sensitive-value-must-not-be-logged" in invalid_text:
        failures.append("invalid sensitive fixture failure must not echo the fixture secret value")

    result = {
        "status": "fail" if failures else "pass",
        "template_loaded": True,
        "example_loaded": True,
        "invalid_sensitive_rejected": bool(invalid_failures),
        "references_only_no_secret_values": True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
