#!/usr/bin/env python3
"""Guard the official SDK sign-only regression suite."""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "lib.rs"
GATES = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST = ROOT / "validation" / "write_v0_23_evidence_manifest.py"
DOC = ROOT / "docs" / "SDK_REGRESSION_SUITE.md"

REQUIRED_ADAPTER_TOKENS = [
    "standard_sign_only_profile_is_non_posting_v2_pusd",
    "standard_sign_only_plan_is_default_sdk_construct_path_without_raw_payload",
    "standard_sign_only_construction_emits_only_digest_ref_and_lifecycle",
    "standard_sign_only_construction_ref_is_stable_for_same_mapping",
    "standard_sign_only_plan_rejects_profile_that_can_post_or_expose_raw_order",
    "plan_mapping_normalizes_limit_orders",
    "plan_mapping_supports_market_amount",
    "plan_mapping_maps_ioc_to_sdk_fak",
    "plan_mapping_supports_fok_limit_orders",
    "plan_mapping_rejects_gtd_until_expiration_path_exists",
    "plan_mapping_preserves_metadata_without_exposing_signed_payload",
    "plan_mapping_rejects_placeholder_token_id",
    "plan_mapping_rejects_invalid_limit_price_and_zero_size",
    "redacts_private_key_like_hex_tokens",
    "gateway_error_conversion_redacts_sensitive_message",
    "normalized_error_redaction_covers_remote_unknown_messages",
    "sdk_error_normalization_covers_validation",
    "sdk_error_normalization_covers_status_codes",
    "gateway_error_conversion_preserves_remote_unknown",
    "geoblock_status_maps_to_core_status",
    "read_only_smoke_ignores_ambient_credentials_but_must_remain_unauthenticated",
    "authenticated_non_trading_is_explicit_opt_in",
    "sign_only_is_not_live_submit",
]

REQUIRED_DOC_TOKENS = [
    "mapping snapshot",
    "redaction",
    "error normalization",
    "geoblock",
    "read-only authenticated smoke",
    "sign-only dry-run",
    "no remote trading side effect",
]

FORBIDDEN_SIDE_EFFECT_CALLS = [
    re.compile(r"\.\s*post_order\s*\("),
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_order\s*\("),
    re.compile(r"\.\s*cancel_orders\s*\("),
]


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"//.*", "", text)
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def main() -> int:
    failures: list[str] = []
    adapter = ADAPTER.read_text()
    stripped_adapter = strip_rust_comments(adapter)
    for token in REQUIRED_ADAPTER_TOKENS:
        if token not in adapter:
            failures.append(f"adapter regression suite missing token: {token}")
    for pattern in FORBIDDEN_SIDE_EFFECT_CALLS:
        if pattern.search(stripped_adapter):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")

    if not DOC.exists():
        failures.append("SDK regression suite document missing")
    else:
        doc = DOC.read_text().lower()
        for token in REQUIRED_DOC_TOKENS:
            if token not in doc:
                failures.append(f"SDK regression suite document missing token: {token}")

    gates = GATES.read_text()
    manifest = MANIFEST.read_text()
    if "37-sdk-regression-suite-guard.log" not in gates:
        failures.append("run_v0_23_gates.sh must emit SDK regression suite guard log")
    if '"sdk_regression_suite_validation"' not in manifest:
        failures.append("evidence manifest must include sdk_regression_suite_validation")
    if "37-sdk-regression-suite-guard.log" not in manifest:
        failures.append("evidence manifest must capture SDK regression suite guard log")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("SDK regression suite guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
