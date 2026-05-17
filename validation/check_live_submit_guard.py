#!/usr/bin/env python3
"""Static guard for pre-live execution releases.

This check is deliberately conservative: the official SDK adapter may contain documentation,
feature-gate names, and validation helpers for future live submit, but it must not contain an
actual SDK post_order invocation in pre-live releases. Fake gateway tests live in pmx-gateway and
are intentionally out of scope.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "lib.rs"
PUBLIC_CONTRACT = ROOT / "openapi" / "executor.v1.yaml"

FORBIDDEN_ADAPTER_PATTERNS = [
    re.compile(r"\.\s*post_order\s*\("),
    re.compile(r"\.\s*post_orders\s*\("),
]
FORBIDDEN_PUBLIC_TERMS = [
    "SignedOrderEnvelope",
    "signed_payload",
    "private_key",
    "clob_secret",
    "post_order",
]


def strip_rust_comments(text: str) -> str:
    # Good enough for the current guard. It prevents docs/comments containing post_order from
    # tripping the check while still catching live SDK method calls.
    text = re.sub(r"//.*", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return text


def main() -> int:
    adapter_text = strip_rust_comments(ADAPTER.read_text())
    failures: list[str] = []
    for pattern in FORBIDDEN_ADAPTER_PATTERNS:
        if pattern.search(adapter_text):
            failures.append(f"official SDK adapter contains forbidden call pattern: {pattern.pattern}")

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
