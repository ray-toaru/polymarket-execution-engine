#!/usr/bin/env python3
"""Guard official SDK standard sign-only adapter boundaries."""
from __future__ import annotations

import re
import sys
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
ALLOWED_CANARY_POST_ORDER = ADAPTER / "sdk_runtime" / "live_canary.rs"

REQUIRED = [
    "OfficialSdkStandardSignOnlyProfile",
    "OfficialSdkStandardSignOnlyPlan",
    "OfficialSdkStandardSignOnlyConstruction",
    "standard_sign_only_plan_for_order",
    "standard_sign_only_default_plan_for_order",
    "standard_sign_only_construction_for_order",
    "standard_sign_only_digest",
    "CLOB_V2_COLLATERAL_SYMBOL",
    "CLOB_V2_SIGNING_PROTOCOL",
    "uses_deposit_wallet_order_path",
    "exposes_raw_signed_order: false",
    "may_post_order: false",
    "may_cancel_order: false",
    "validate_standard_sign_only_profile",
    "plan_mapping_maps_ioc_to_sdk_fak",
    "plan_mapping_preserves_metadata_without_exposing_signed_payload",
    "standard_sign_only_profile_is_non_posting_v2_pusd",
    "standard_sign_only_plan_is_default_sdk_construct_path_without_raw_payload",
    "standard_sign_only_construction_emits_only_digest_ref_and_lifecycle",
    "standard_sign_only_construction_ref_is_stable_for_same_mapping",
    "standard_sign_only_plan_rejects_profile_that_can_post_or_expose_raw_order",
    "plan_mapping_supports_fok_limit_orders",
    "plan_mapping_supports_gtd_with_expiration",
    "plan_mapping_rejects_gtd_without_valid_expiration",
    "expiration",
    "builder_attribution",
    "fee_rate_bps",
    "funder",
    "signer",
    "signature_type",
]

FORBIDDEN_PATTERNS = [
    re.compile(r"\.\s*post_orders\s*\("),
    re.compile(r"\.\s*cancel_order\s*\("),
    re.compile(r"\.\s*cancel_orders\s*\("),
]
POST_ORDER_CALL = re.compile(r"\.\s*post_order\s*\(")


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"//.*", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return text


def read_rust_sources(path: Path) -> str:
    return "\n".join(source.read_text() for source in sorted(path.rglob("*.rs")))


def rust_sources(path: Path) -> list[Path]:
    return sorted(path.rglob("*.rs"))


def main() -> int:
    failures: list[str] = []
    adapter = read_rust_sources(ADAPTER)
    stripped = strip_rust_comments(adapter)
    for needle in REQUIRED:
        if needle not in adapter:
            failures.append(f"adapter missing {needle}")
    for pattern in FORBIDDEN_PATTERNS:
        if pattern.search(stripped):
            failures.append(f"adapter contains forbidden remote side-effect call: {pattern.pattern}")
    post_order_call_sites = [
        source
        for source in rust_sources(ADAPTER)
        if POST_ORDER_CALL.search(strip_rust_comments(source.read_text()))
    ]
    if post_order_call_sites != [ALLOWED_CANARY_POST_ORDER]:
        display = ", ".join(str(path.relative_to(ADAPTER)) for path in post_order_call_sites) or "none"
        failures.append(
            "post_order call sites must be limited to guarded real-funds canary, "
            f"not standard sign-only paths; found {display}"
        )
    if ALLOWED_CANARY_POST_ORDER.exists():
        canary = strip_rust_comments(ALLOWED_CANARY_POST_ORDER.read_text())
        for token in [
            "validate_real_funds_canary_preconditions",
            "SdkOrderType::FOK",
            "raw_signed_order_exposed: false",
        ]:
            if token not in canary:
                failures.append(f"guarded canary post_order site missing token: {token}")
    manifest = MANIFEST.read_text()
    require_current_gate_log("35-sdk-standard-sign-only-guard.log", "SDK standard sign-only guard", failures)
    if '"sdk_standard_sign_only_validation"' not in manifest:
        failures.append("evidence manifest must include sdk_standard_sign_only_validation")
    if "35-sdk-standard-sign-only-guard.log" not in manifest:
        failures.append("evidence manifest must capture SDK standard sign-only guard log")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("SDK standard sign-only guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
