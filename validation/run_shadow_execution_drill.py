#!/usr/bin/env python3
"""Run a non-posting shadow execution drill against public market data.

The drill intentionally performs only unauthenticated public reads and local
decision construction. It must not sign, post, cancel, or expose credentials.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

DEFAULT_CLOB_HOST = "https://clob-v2.polymarket.com"
DEFAULT_MARKET_LIMIT = 200
DEFAULT_SIZE = "5"
DEFAULT_LIMIT_PRICE = "0.01"
FORBIDDEN_ENV_NAMES = (
    "POLYMARKET_PRIVATE_KEY",
    "POLY_API_KEY",
    "POLY_API_SECRET",
    "POLY_API_PASSPHRASE",
)


def fail(message: str) -> int:
    print(json.dumps({"status": "fail", "reason": message}, sort_keys=True))
    return 1


def skipped(message: str) -> int:
    print(json.dumps({"status": "skipped", "skipped_reason": message}, sort_keys=True))
    return 0


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def decimal_string(value: str, name: str) -> str:
    try:
        parsed = Decimal(value)
    except InvalidOperation as exc:
        raise ValueError(f"{name} must be a decimal string") from exc
    if parsed <= 0:
        raise ValueError(f"{name} must be positive")
    return format(parsed.normalize(), "f")


def fetch_json(url: str) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": "pmx-shadow-validation/0.24",
        },
        method="GET",
    )
    with urllib.request.urlopen(request, timeout=15) as response:
        return json.loads(response.read().decode("utf-8"))


def active_market(markets: list[dict[str, Any]]) -> dict[str, Any]:
    for market in markets:
        if not (
            market.get("active")
            and not market.get("archived")
            and market.get("accepting_orders")
        ):
            continue
        tokens = [token for token in market.get("tokens", []) if token.get("token_id")]
        if tokens:
            return {**market, "tokens": tokens}
    raise ValueError("no active accepting simplified market with token_id found")


def main() -> int:
    if os.environ.get("PMX_RUN_SHADOW_EXECUTION_DRILL") != "1":
        return skipped("PMX_RUN_SHADOW_EXECUTION_DRILL not set to 1")
    if os.environ.get("PMX_ALLOW_LIVE_SUBMIT") == "1":
        return fail("PMX_ALLOW_LIVE_SUBMIT=1 is incompatible with shadow drill")

    clob_host = os.environ.get("PMX_SHADOW_CLOB_HOST", DEFAULT_CLOB_HOST).rstrip("/")
    limit = int(os.environ.get("PMX_SHADOW_MARKET_LIMIT", str(DEFAULT_MARKET_LIMIT)))
    size = decimal_string(os.environ.get("PMX_SHADOW_SIZE", DEFAULT_SIZE), "PMX_SHADOW_SIZE")
    limit_price = decimal_string(
        os.environ.get("PMX_SHADOW_LIMIT_PRICE", DEFAULT_LIMIT_PRICE),
        "PMX_SHADOW_LIMIT_PRICE",
    )
    url = f"{clob_host}/simplified-markets?limit={limit}"

    try:
        payload = fetch_json(url)
        market = active_market(payload.get("data", []))
    except (urllib.error.URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
        return fail(f"public market read failed: {exc}")

    token = market["tokens"][0]
    token_id = str(token["token_id"])
    condition_id = str(market.get("condition_id") or "")
    trace_seed = json.dumps(
        {
            "condition_id": condition_id,
            "token_id": token_id,
            "side": "BUY",
            "size": size,
            "limit_price": limit_price,
        },
        sort_keys=True,
    )
    trace_id = f"shadow-{sha256_text(trace_seed)[:24]}"
    decision = {
        "schema_version": 1,
        "status": "pass",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "drill": "shadow_execution_would_submit",
        "remote_methods": ["GET /simplified-markets"],
        "remote_side_effects": False,
        "posted": False,
        "signed": False,
        "cancelled": False,
        "live_submit_enabled": False,
        "trace_id": trace_id,
        "market": {
            "condition_id_hash": sha256_text(condition_id),
            "active": bool(market.get("active")),
            "closed": bool(market.get("closed")),
            "archived": bool(market.get("archived")),
            "accepting_orders": bool(market.get("accepting_orders")),
        },
        "would_submit": {
            "decision": "WOULD_SUBMIT_BLOCKED_NON_POSTING_SHADOW",
            "reason": "shadow drill constructs a candidate order but does not sign or submit",
            "order": {
                "token_id_hash": sha256_text(token_id),
                "side": "BUY",
                "order_kind": "LIMIT",
                "size": size,
                "limit_price": limit_price,
                "time_in_force": "GTC",
                "post_only": False,
            },
        },
        "safety": {
            "sensitive_env_present": any(bool(os.environ.get(name)) for name in FORBIDDEN_ENV_NAMES),
            "credentials_logged": False,
            "raw_signed_payload_logged": False,
            "raw_signature_logged": False,
        },
    }
    print(json.dumps(decision, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
