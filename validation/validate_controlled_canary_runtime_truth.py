#!/usr/bin/env python3
"""Validate controlled-canary runtime-truth references are complete and reference-only."""
from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config"
TEMPLATE = CONFIG / "controlled-canary.runtime-truth.template.json"
INVALID_PARTIAL = CONFIG / "controlled-canary.runtime-truth.invalid-partial.fixture.json"
INVALID_SENSITIVE = CONFIG / "controlled-canary.runtime-truth.invalid-sensitive.fixture.json"

REQUIRED_DEPENDENCIES = {
    "kill_switch",
    "live_submit_gate",
    "idempotency_lease",
    "order_cancel_reconciliation",
}
AUTHORIZATION_FLAGS = [
    "live_submit_allowed",
    "live_cancel_allowed",
    "real_funds_canary_authorized",
    "remote_side_effects",
    "production_ready_claimed",
]
PREFLIGHT_BOOL_FIELDS = [
    "posted",
    "remote_side_effects",
    "raw_signed_order_exposed",
    "live_submit_allowed",
    "real_funds_canary_allowed",
    "kill_switch_open",
    "runtime_worker_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_worker_healthy",
    "cancel_only_fallback_ready",
    "balance_allowance_checked",
]
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


def expected_source_release() -> str:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    return f"v{cargo['workspace']['package']['version']}"


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
                    failures.append(f"forbidden sensitive runtime-truth key: {child_path}")
                walk(child, child_path)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                walk(child, f"{path}[{index}]")
        elif isinstance(value, str):
            if any(fragment in value for fragment in FORBIDDEN_VALUE_FRAGMENTS):
                failures.append(f"forbidden sensitive-looking runtime-truth value: {path}")

    walk(data, "")
    return failures


def validate_shape(data: dict[str, Any], label: str, *, allow_placeholders: bool) -> list[str]:
    failures: list[str] = []
    if data.get("schema_version") != 1:
        failures.append(f"{label}: schema_version must be 1")
    source_release = expected_source_release()
    if data.get("source_release") != source_release:
        failures.append(f"{label}: source_release must be {source_release}")
    if data.get("scope") != "REAL_FUNDS_CANARY":
        failures.append(f"{label}: scope must be REAL_FUNDS_CANARY")
    if data.get("execution_style") != "GTC_LIMIT_POST_ONLY_CANCEL":
        failures.append(f"{label}: execution_style must be GTC_LIMIT_POST_ONLY_CANCEL")
    if not isinstance(data.get("account_id"), str) or not data.get("account_id", "").strip():
        failures.append(f"{label}: account_id must be a non-empty string")
    if not isinstance(data.get("condition_id"), str) or not data.get("condition_id", "").strip():
        failures.append(f"{label}: condition_id must be a non-empty string")
    if data.get("references_only_no_secret_values") is not True:
        failures.append(f"{label}: references_only_no_secret_values must be true")
    for flag in AUTHORIZATION_FLAGS:
        if data.get(flag) is not False:
            failures.append(f"{label}: {flag} must be false")
    failures.extend(f"{label}: {failure}" for failure in validate_no_sensitive_material(data))

    preflight_report = data.get("preflight_report")
    if not isinstance(preflight_report, dict):
        failures.append(f"{label}: preflight_report must be an object")
    else:
        if preflight_report.get("status") != "preflight_ready":
            failures.append(f"{label}: preflight_report.status must be preflight_ready")
        for field in PREFLIGHT_BOOL_FIELDS:
            if not isinstance(preflight_report.get(field), bool):
                failures.append(f"{label}: preflight_report.{field} must be boolean")

    if not allow_placeholders:
        for field in ["artifact_sha256", "workspace_manifest_sha256", "archived_manifest_sha256"]:
            if not is_sha256(data.get(field)):
                failures.append(f"{label}: {field} must be 64-hex")
    else:
        for field in ["artifact_sha256", "workspace_manifest_sha256", "archived_manifest_sha256"]:
            value = data.get(field)
            if not has_placeholder(value) and not is_sha256(value):
                failures.append(f"{label}: {field} must be placeholder or 64-hex")

    dependencies = data.get("dependencies")
    if not isinstance(dependencies, list):
        failures.append(f"{label}: dependencies must be a list")
        return failures
    dependency_by_name: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(dependencies):
        if not isinstance(item, dict):
            failures.append(f"{label}: dependencies[{index}] must be an object")
            continue
        name = item.get("name")
        if not isinstance(name, str) or not name:
            failures.append(f"{label}: dependencies[{index}].name is required")
            continue
        if name in dependency_by_name:
            failures.append(f"{label}: duplicate dependency {name}")
        dependency_by_name[name] = item
    missing = sorted(REQUIRED_DEPENDENCIES - set(dependency_by_name))
    if missing:
        failures.append(f"{label}: runtime truth missing durable dependencies: {', '.join(missing)}")
    for name, item in sorted(dependency_by_name.items()):
        if item.get("status") != "durable_runtime_truth":
            failures.append(f"{label}: dependency {name} must have status=durable_runtime_truth")
        evidence_ref = item.get("evidence_ref")
        if not isinstance(evidence_ref, str) or not evidence_ref.strip():
            failures.append(f"{label}: dependency {name} evidence_ref is required")
        elif not allow_placeholders and has_placeholder(evidence_ref):
            failures.append(f"{label}: dependency {name} evidence_ref still has placeholder")
        elif allow_placeholders and not (has_placeholder(evidence_ref) or evidence_ref.strip()):
            failures.append(f"{label}: dependency {name} evidence_ref must be placeholder or concrete reference")
    return failures


def validate_file(path: Path, *, allow_placeholders: bool) -> dict[str, Any]:
    data = load(path)
    failures = validate_shape(data, str(path), allow_placeholders=allow_placeholders)
    placeholders = placeholder_paths(data)
    return {
        "status": "fail" if failures else "pass",
        "file": str(path),
        "allow_placeholders": allow_placeholders,
        "placeholders_remaining": placeholders,
        "references_only_no_secret_values": data.get("references_only_no_secret_values") is True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "failures": failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--file",
        type=Path,
        help="Validate an operator-supplied runtime truth candidate instead of built-in fixtures.",
    )
    parser.add_argument(
        "--allow-placeholders",
        action="store_true",
        help="Allow REPLACE_WITH_* placeholders for generated local review material.",
    )
    args = parser.parse_args()

    if args.file:
        result = validate_file(args.file, allow_placeholders=args.allow_placeholders)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result["status"] == "pass" else 1

    failures: list[str] = []
    for path in [TEMPLATE, INVALID_PARTIAL, INVALID_SENSITIVE]:
        if not path.exists():
            failures.append(f"missing {path.relative_to(ROOT)}")
    if failures:
        print(json.dumps({"status": "fail", "failures": failures}, indent=2, sort_keys=True))
        return 1

    template_failures = validate_shape(load(TEMPLATE), "template", allow_placeholders=True)
    if template_failures:
        failures.extend(template_failures)

    invalid_partial_failures = validate_shape(load(INVALID_PARTIAL), "invalid partial fixture", allow_placeholders=False)
    if not any("runtime truth missing durable dependencies" in failure for failure in invalid_partial_failures):
        failures.append("invalid partial fixture must be rejected for missing durable dependencies")

    invalid_sensitive_failures = validate_shape(load(INVALID_SENSITIVE), "invalid sensitive fixture", allow_placeholders=False)
    if not any("forbidden sensitive" in failure for failure in invalid_sensitive_failures):
        failures.append("invalid sensitive fixture must be rejected for sensitive material")

    print(
        json.dumps(
            {
                "status": "fail" if failures else "ok",
                "template": str(TEMPLATE),
                "required_dependencies": sorted(REQUIRED_DEPENDENCIES),
                "failures": failures,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
