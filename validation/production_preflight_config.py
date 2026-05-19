"""Shared production-preflight config loading and redaction checks."""
from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONFIG = ROOT / "config" / "production-preflight.example.json"

FORBIDDEN_KEYS = {
    "private_key",
    "privateKey",
    "clob_secret",
    "clobSecret",
    "secret",
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
    "clob_secret=",
    "raw_signature=",
    "raw_signed_payload=",
)


def configured_path(*, use_default: bool = False) -> Path | None:
    raw = os.environ.get("PMX_PRODUCTION_PREFLIGHT_CONFIG", "").strip()
    if not raw:
        return DEFAULT_CONFIG if use_default else None
    path = Path(raw)
    if not path.is_absolute():
        path = (ROOT / path).resolve()
    return path


def load_config(*, use_default: bool = False) -> tuple[dict[str, Any], Path | None, list[str]]:
    path = configured_path(use_default=use_default)
    if path is None:
        return {}, None, []
    if not path.exists():
        if "PMX_PRODUCTION_PREFLIGHT_CONFIG" in os.environ:
            return {}, path, [f"configured preflight config not found: {path}"]
        return {}, None, []
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        return {}, path, [f"invalid preflight config JSON: {exc}"]
    if not isinstance(data, dict):
        return {}, path, ["preflight config root must be an object"]
    failures = validate_no_sensitive_material(data)
    if data.get("schema_version") != 1:
        failures.append("preflight config schema_version must be 1")
    return data, path, failures


def validate_no_sensitive_material(data: object) -> list[str]:
    failures: list[str] = []

    def walk(value: object, path: str) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                child_path = f"{path}.{key}" if path else str(key)
                if key in FORBIDDEN_KEYS:
                    failures.append(f"forbidden sensitive config key: {child_path}")
                walk(child, child_path)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                walk(child, f"{path}[{index}]")
        elif isinstance(value, str):
            if any(fragment in value for fragment in FORBIDDEN_VALUE_FRAGMENTS):
                failures.append(f"forbidden sensitive-looking config value: {path}")

    walk(data, "")
    return failures


def nested_present(data: dict[str, Any], section: str, field: str) -> bool:
    section_value = data.get(section)
    if not isinstance(section_value, dict):
        return False
    value = section_value.get(field)
    return isinstance(value, str) and bool(value.strip())
