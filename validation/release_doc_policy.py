from __future__ import annotations

import re


STALE_ROOT_DOC_PATTERNS = (
    re.compile(r"^V0_.*\.md$", re.IGNORECASE),
    re.compile(r"^VALIDATION_V0_.*\.md$", re.IGNORECASE),
    re.compile(r".*_GATE_CONFIRMATION\.md$", re.IGNORECASE),
    re.compile(r"^VALIDATION_CONFIRMATION_REPORT\.md$", re.IGNORECASE),
    re.compile(r"^CONTINUATION_REPORT\.md$", re.IGNORECASE),
    re.compile(r"^ISSUES_CONFIRMED_AND_FIXED\.md$", re.IGNORECASE),
)

HISTORICAL_ROOT_DOC_PATTERNS = (
    re.compile(r"\bHistorical\s+v0(?:[._]\d+|\.\d+(?:\.\d+)?)\b", re.IGNORECASE),
    re.compile(r"\bhistorical\s+(?:continuation|review|gate-confirmation)\b", re.IGNORECASE),
)

RELEASE_SPECIFIC_AGENT_PATTERNS = (
    re.compile(r"\brun_v0_\d+_gates\.sh\b", re.IGNORECASE),
    re.compile(
        r"\b(?:current|target|validated|production|live|release|promotion|candidate)\b"
        r"[^\n]{0,48}\b(?:v0(?:\.\d+(?:\.\d+)?)?|v0_\d+|V0_\d+|0\.\d+(?:\.\d+)?)\b",
        re.IGNORECASE,
    ),
    re.compile(
        r"\b(?:v0(?:\.\d+(?:\.\d+)?)?|v0_\d+|V0_\d+|0\.\d+(?:\.\d+)?)\b"
        r"[^\n]{0,48}\b(?:release|ready|candidate|promotion|production|live|validated|current)\b",
        re.IGNORECASE,
    ),
)


def contains_historical_root_doc_marker(text: str) -> bool:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    window = "\n".join(lines[:5])
    return any(pattern.search(window) for pattern in HISTORICAL_ROOT_DOC_PATTERNS)


def contains_release_specific_agents_marker(text: str) -> bool:
    return any(pattern.search(text) for pattern in RELEASE_SPECIFIC_AGENT_PATTERNS)
