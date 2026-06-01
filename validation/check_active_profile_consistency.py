#!/usr/bin/env python3
"""Validate a runtime-facing active account env file for the real-funds canary path.

``PMX_ACTIVE_ACCOUNT_PROFILE`` is the local profile selector. ``PMX_ACTIVE_ACCOUNT_ID``
and ``PMX_ACTIVE_PROFILE_REF`` are opaque runtime identity values and are checked
for presence/equality only; this guard does not normalize their spelling.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shlex


ROOT = Path(__file__).resolve().parents[2]
SIGNATURE_TYPE_ALIASES = {
    "EOA": "EOA",
    "0": "EOA",
    "POLY_1271": "POLY_1271",
    "POLY1271": "POLY_1271",
    "DEPOSIT_WALLET": "POLY_1271",
    "3": "POLY_1271",
}
RUNTIME_REQUIRED = [
    "PMX_ACTIVE_ACCOUNT_PROFILE",
    "PMX_ACTIVE_ACCOUNT_ID",
    "PMX_ACTIVE_PROFILE_REF",
    "POLYMARKET_PRIVATE_KEY",
    "POLY_API_KEY",
    "POLY_API_SECRET",
    "POLY_API_PASSPHRASE",
    "PMX_CLOB_SIGNATURE_TYPE",
]

UNSUPPORTED_ENV_TOKENS = ("`", "$(", "${", "&&", "||", ";")


def parse_env_value(raw_value: str, *, path: Path, raw_line: str) -> str:
    value = raw_value
    if any(token in value for token in UNSUPPORTED_ENV_TOKENS):
        raise SystemExit(f"unsupported shell-style env value in {path}: {raw_line}")
    stripped = value.strip()
    if not stripped:
        return ""
    if stripped[0] in {"'", '"'}:
        try:
            parsed = shlex.split(stripped, posix=True)
        except ValueError as exc:
            raise SystemExit(f"invalid quoted env value in {path}: {raw_line}") from exc
        if len(parsed) != 1:
            raise SystemExit(f"invalid quoted env value in {path}: {raw_line}")
        return parsed[0]
    return value


def companion_secrets_path(path: Path) -> Path:
    if path.suffix == ".example":
        return path.with_name(path.stem + ".secrets" + path.suffix)
    return path.with_name(path.name + ".secrets")


def parse_env_file(path: Path) -> tuple[dict[str, str], list[str]]:
    values: dict[str, str] = {}
    raw_keys: list[str] = []
    for raw_line in path.read_text().splitlines():
        stripped = raw_line.lstrip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith("export "):
            raise SystemExit(f"unsupported export syntax in {path}: {raw_line}")
        if "=" not in raw_line:
            raise SystemExit(f"invalid env assignment in {path}: {raw_line}")
        key, value = raw_line.split("=", 1)
        key = key.strip()
        raw_keys.append(key)
        values[key] = parse_env_value(value, path=path, raw_line=raw_line)
    return values, raw_keys


def normalize_signature_type(raw: str) -> str:
    normalized = raw.strip().upper()
    try:
        return SIGNATURE_TYPE_ALIASES[normalized]
    except KeyError as exc:
        raise SystemExit(
            "PMX_CLOB_SIGNATURE_TYPE must be EOA or POLY_1271; numeric aliases 0 and 3 are accepted"
        ) from exc


def evaluate_env_file(path: Path, expected_account_id: str | None = None) -> dict[str, str]:
    values, raw_keys = parse_env_file(path)
    if companion_secrets_path(path).is_file():
        companion_values, companion_raw_keys = parse_env_file(companion_secrets_path(path))
        values.update(companion_values)
        raw_keys.extend(companion_raw_keys)
    forbidden = [
        key for key in raw_keys if key.startswith("PMX_PROFILE_") or key.startswith("PMX_ACCT_")
    ]
    if forbidden:
        raise SystemExit(
            "runtime env file must not contain profile source variables: "
            + ", ".join(sorted(forbidden))
        )
    missing = [key for key in RUNTIME_REQUIRED if not values.get(key, "").strip()]
    if missing:
        raise SystemExit("missing required runtime env variables: " + ", ".join(missing))
    signature_type = normalize_signature_type(values["PMX_CLOB_SIGNATURE_TYPE"])
    funder = values.get("PMX_CLOB_FUNDER", "").strip()
    if signature_type == "POLY_1271" and not funder:
        raise SystemExit("PMX_CLOB_FUNDER is required when PMX_CLOB_SIGNATURE_TYPE=POLY_1271")
    if expected_account_id and values["PMX_ACTIVE_ACCOUNT_ID"] != expected_account_id:
        raise SystemExit(
            f"active account id mismatch: expected {expected_account_id} got {values['PMX_ACTIVE_ACCOUNT_ID']}"
        )
    return {
        "status": "pass",
        "active_account_profile": values["PMX_ACTIVE_ACCOUNT_PROFILE"],
        "active_account_id": values["PMX_ACTIVE_ACCOUNT_ID"],
        "active_profile_ref": values["PMX_ACTIVE_PROFILE_REF"],
        "signature_type": signature_type,
        "has_funder": "true" if bool(funder) else "false",
        "env_file": str(path),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--env-file", required=True, type=Path)
    parser.add_argument("--expected-account-id")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    env_file = args.env_file if args.env_file.is_absolute() else ROOT / args.env_file
    report = evaluate_env_file(env_file, expected_account_id=args.expected_account_id)
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
