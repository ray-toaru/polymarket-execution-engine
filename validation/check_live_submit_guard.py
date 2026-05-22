#!/usr/bin/env python3
"""Static guard for pre-live and guarded-canary execution releases.

This check is deliberately conservative: the official SDK adapter may contain documentation,
feature-gate names, validation helpers, and one guarded real-funds canary submit call site behind
the live-submit feature. Fake gateway tests live in pmx-gateway and are intentionally out of scope.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER_SRC = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
SERVICE_SRC = ROOT / "crates" / "pmx-service" / "src"
STORE_SRC = ROOT / "crates" / "pmx-store" / "src"
PUBLIC_CONTRACT = ROOT / "openapi" / "executor.v1.yaml"

ALLOWED_GATEWAY_POST_ORDER_FILE = ADAPTER_SRC / "sdk_runtime" / "gateway.rs"
ALLOWED_POST_ORDER_FILE = ADAPTER_SRC / "sdk_runtime" / "live_canary.rs"
ALLOWED_SERVICE_POST_ORDER_FILE = SERVICE_SRC / "submit" / "live.rs"
FORBIDDEN_BULK_POST_ORDER = re.compile(r"\.\s*post_orders\s*\(")
POST_ORDER_CALL = re.compile(r"\.\s*post_order\s*\(")
MARKET_ORDER_CALL = re.compile(r"\.\s*market_order\s*\(")
FORBIDDEN_PUBLIC_TERMS = [
    "SignedOrderEnvelope",
    "signed_payload",
    "private_key",
    "clob_secret",
    "post_order",
]
REQUIRED_CANARY_TOKENS = [
    "LiveCanaryPreconditions",
    "default_blocked_live_canary_preconditions",
    "validate_live_submit_canary_preconditions",
    "compile_feature_live_submit",
    "env_allow_live_submit",
    "config_allow_live_submit",
    "kill_switch_open",
    "runtime_worker_healthy",
    "geoblock_allowed",
    "repository_reservation_exists",
    "idempotency_key_written",
    "reconcile_worker_healthy",
    "account_whitelisted",
    "market_whitelisted",
    "size_cap_ok",
    "daily_cap_ok",
    "operator_approved",
    "cancel_only_fallback_ready",
    "live_submit_canary_requires_every_gate",
    "live_canary_default_preconditions_are_blocked_without_side_effects",
    "live_submit_canary_requires_cancel_only_fallback",
    "RealFundsCanaryPreconditions",
    "ENV_ALLOW_REAL_FUNDS_CANARY",
    "validate_real_funds_canary_preconditions",
    "run_real_funds_canary_gtc_post_only_cancel",
    "SdkOrderType::GTC",
    "raw_signed_order_exposed: false",
]
REQUIRED_IDEMPOTENCY_TOKENS = [
    (
        STORE_SRC / "memory" / "idempotency.rs",
        [
            "IDEMPOTENCY_LEASE_SECS",
            "lease_expires_at > now",
            "IdempotencyAction::InProgress",
            "existing.owner_token = owner_token.clone()",
            "record.owner_token != attempt.owner_token",
        ],
    ),
    (
        STORE_SRC / "postgres_idempotency" / "begin.rs",
        [
            "IDEMPOTENCY_LEASE_SECS",
            "expires_at > now",
            "IdempotencyAction::InProgress",
            "owner_token = format!(\"owner-{}\", Uuid::new_v4())",
            "response_fingerprint = NULL, response_json = NULL",
        ],
    ),
    (
        STORE_SRC / "postgres_idempotency" / "finish.rs",
        [
            "existing_owner.as_deref() != Some(attempt.owner_token)",
            "AND owner_token = $4 AND status = 'PROCEEDING'",
            "idempotency finish lost owner_token race",
        ],
    ),
]


def strip_rust_comments(text: str) -> str:
    # Good enough for the current guard. It prevents docs/comments containing post_order from
    # tripping the check while still catching live SDK method calls.
    text = re.sub(r"//.*", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return text


def read_adapter_sources() -> str:
    return "\n".join(path.read_text() for path in sorted(ADAPTER_SRC.rglob("*.rs")))


def adapter_source_files() -> list[Path]:
    return sorted(ADAPTER_SRC.rglob("*.rs"))


def service_source_files() -> list[Path]:
    return [
        path
        for path in sorted(SERVICE_SRC.rglob("*.rs"))
        if "service_tests" not in path.parts
    ]


def main() -> int:
    raw_adapter_text = read_adapter_sources()
    adapter_text = strip_rust_comments(raw_adapter_text)
    failures: list[str] = []
    if FORBIDDEN_BULK_POST_ORDER.search(adapter_text):
        failures.append("official SDK adapter contains forbidden bulk post_orders call pattern")
    post_order_call_sites: list[Path] = []
    for path in adapter_source_files():
        text = strip_rust_comments(path.read_text())
        if POST_ORDER_CALL.search(text):
            post_order_call_sites.append(path)
    allowed_post_order_sites = [ALLOWED_GATEWAY_POST_ORDER_FILE, ALLOWED_POST_ORDER_FILE]
    if post_order_call_sites != allowed_post_order_sites:
        display = ", ".join(str(path.relative_to(ADAPTER_SRC)) for path in post_order_call_sites) or "none"
        failures.append(f"official SDK adapter post_order call sites must be limited to sdk_runtime/live_canary.rs; found {display}")
    if ALLOWED_POST_ORDER_FILE.exists():
        allowed_text = strip_rust_comments(ALLOWED_POST_ORDER_FILE.read_text())
        if ".post_order(signed)" not in allowed_text:
            failures.append("allowed live canary post_order call must submit only the locally signed canary order")
        if MARKET_ORDER_CALL.search(allowed_text):
            failures.append("real-funds canary must not use market_order amount semantics")
        for token in [
            "validate_real_funds_canary_preconditions",
            "limit_order()",
            "size(size)",
            "SdkOrderType::GTC",
            "raw_signed_order_exposed: false",
        ]:
            if token not in allowed_text:
                failures.append(f"allowed live canary post_order file missing token: {token}")
    service_post_order_call_sites: list[Path] = []
    for path in service_source_files():
        text = strip_rust_comments(path.read_text())
        if POST_ORDER_CALL.search(text):
            service_post_order_call_sites.append(path)
    if service_post_order_call_sites != [ALLOWED_SERVICE_POST_ORDER_FILE]:
        display = ", ".join(str(path.relative_to(SERVICE_SRC)) for path in service_post_order_call_sites) or "none"
        failures.append(f"pmx-service post_order call sites must be limited to submit/live.rs; found {display}")
    service_text = "\n".join(path.read_text() for path in service_source_files())
    if "submit_plan_with_gateway" not in service_text:
        failures.append("pmx-service live gateway path must require explicit submit_plan_with_gateway")
    if ALLOWED_SERVICE_POST_ORDER_FILE.exists():
        service_live_text = strip_rust_comments(ALLOWED_SERVICE_POST_ORDER_FILE.read_text())
        for token in [
            "LIVE_SUBMIT_BLOCKED_PRE_SIGN_RUNTIME",
            "LIVE_SUBMIT_BLOCKED_PRE_POST_RUNTIME",
            "runtime_submit_block_reason",
            "raw_signed_payload_logged\": false",
            "raw_signed_order_exposed\": false",
        ]:
            if token not in service_live_text:
                failures.append(f"pmx-service live submit path missing token: {token}")
    submit_text = (SERVICE_SRC / "submit.rs").read_text()
    for token in [
        "LIVE submit mode is fail-closed until gateway posting is wired through the executor service",
        "submit_plan_with_gateway",
        "SubmitMode::Live",
    ]:
        if token not in submit_text:
            failures.append(f"pmx-service submit boundary missing token: {token}")
    for path, tokens in REQUIRED_IDEMPOTENCY_TOKENS:
        text = path.read_text()
        for token in tokens:
            if token not in text:
                failures.append(
                    f"idempotency lease/owner guard missing token in {path.relative_to(ROOT)}: {token}"
                )
    for token in REQUIRED_CANARY_TOKENS:
        if token not in raw_adapter_text:
            failures.append(f"official SDK adapter missing live canary guard token: {token}")

    public_text = PUBLIC_CONTRACT.read_text()
    for term in FORBIDDEN_PUBLIC_TERMS:
        if term in public_text:
            failures.append(f"public OpenAPI contract exposes forbidden live/signed term: {term}")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("live-submit static guard passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
