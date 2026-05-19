#!/usr/bin/env python3
"""Validate production preflight config diff review without printing values."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any

from current_gate_chain import require_current_gate_log
from production_preflight_config import load_config

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PRODUCTION_PREFLIGHT_CONFIG_DIFF_REVIEW.md"
MANIFEST = ROOT / "validation" / "write_current_evidence_manifest.py"
DEFAULT_BASELINE = ROOT / "config" / "production-preflight.baseline.fixture.json"
DEFAULT_CANDIDATE = ROOT / "config" / "production-preflight.candidate.fixture.json"
DEFAULT_NEGATIVE = ROOT / "config" / "production-preflight.candidate.invalid-sensitive.fixture.json"
FORBIDDEN_CANDIDATE_VALUE = "candidate-sensitive-value-must-not-be-logged"


def env_enabled(name: str) -> bool:
    return os.environ.get(name, "").strip() == "1"


def path_from_env(name: str, default: Path) -> Path:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    path = Path(raw)
    if not path.is_absolute():
        path = (ROOT / path).resolve()
    return path


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_path(path: Path) -> tuple[dict[str, Any], list[str]]:
    previous = os.environ.get("PMX_PRODUCTION_PREFLIGHT_CONFIG")
    os.environ["PMX_PRODUCTION_PREFLIGHT_CONFIG"] = str(path.relative_to(ROOT))
    try:
        data, _, failures = load_config(use_default=False)
    finally:
        if previous is None:
            os.environ.pop("PMX_PRODUCTION_PREFLIGHT_CONFIG", None)
        else:
            os.environ["PMX_PRODUCTION_PREFLIGHT_CONFIG"] = previous
    return data, failures


def flatten_paths(data: object, prefix: str = "") -> dict[str, str]:
    flattened: dict[str, str] = {}
    if isinstance(data, dict):
        for key, value in data.items():
            path = f"{prefix}.{key}" if prefix else str(key)
            flattened.update(flatten_paths(value, path))
    elif isinstance(data, list):
        for index, value in enumerate(data):
            flattened.update(flatten_paths(value, f"{prefix}[{index}]"))
    else:
        flattened[prefix] = hashlib.sha256(str(data).encode()).hexdigest()
    return flattened


def changed_paths(baseline: dict[str, Any], candidate: dict[str, Any]) -> list[str]:
    left = flatten_paths(baseline)
    right = flatten_paths(candidate)
    keys = sorted(set(left) | set(right))
    return [key for key in keys if left.get(key) != right.get(key)]


def main() -> int:
    failures: list[str] = []
    for env_name in ["PMX_ALLOW_LIVE_SUBMIT", "PMX_ALLOW_LIVE_CANCEL", "PMX_PRODUCTION_READY"]:
        if env_enabled(env_name):
            failures.append(f"{env_name}=1 is forbidden during production preflight config diff review")

    required_tokens = [
        "PMX_PRODUCTION_PREFLIGHT_BASELINE_CONFIG",
        "PMX_PRODUCTION_PREFLIGHT_CANDIDATE_CONFIG",
        "config_diff_review_passed = true",
        "config_diff_review_rejected_sensitive_candidate = true",
        "config_diff_review_secret_value_logged = false",
        "config_diff_review_reports_path_only = true",
        "config_diff_summary_uses_hashes = true",
        "changed_field_paths_present = true",
        "baseline_config_hash_present = true",
        "candidate_config_hash_present = true",
        "live_submit_allowed = false",
        "live_cancel_allowed = false",
        "remote_side_effects = false",
        "production_ready_claimed = false",
    ]
    doc = DOC.read_text() if DOC.exists() else ""
    if not doc:
        failures.append("production preflight config diff review document missing")
    for token in required_tokens:
        if token not in doc:
            failures.append(f"production preflight config diff review document missing token: {token}")

    manifest = MANIFEST.read_text()
    require_current_gate_log("64-production-preflight-config-diff-review.log", "production preflight config diff review", failures)
    if '"production_preflight_config_diff_review_validation"' not in manifest:
        failures.append("evidence manifest must include production_preflight_config_diff_review_validation")
    if "64-production-preflight-config-diff-review.log" not in manifest:
        failures.append("evidence manifest must capture production preflight config diff review log")

    baseline_path = path_from_env("PMX_PRODUCTION_PREFLIGHT_BASELINE_CONFIG", DEFAULT_BASELINE)
    candidate_path = path_from_env("PMX_PRODUCTION_PREFLIGHT_CANDIDATE_CONFIG", DEFAULT_CANDIDATE)
    negative_path = DEFAULT_NEGATIVE
    for label, path in [
        ("baseline", baseline_path),
        ("candidate", candidate_path),
        ("negative candidate", negative_path),
    ]:
        if not path.exists():
            failures.append(f"{label} config missing: {path}")

    baseline, baseline_failures = load_path(baseline_path)
    candidate, candidate_failures = load_path(candidate_path)
    negative, negative_failures = load_path(negative_path)
    del negative
    failures.extend(f"baseline: {failure}" for failure in baseline_failures)
    failures.extend(f"candidate: {failure}" for failure in candidate_failures)
    paths = changed_paths(baseline, candidate) if baseline and candidate else []

    negative_rejected = any("alert_routing.clob_secret" in failure for failure in negative_failures)
    negative_value_logged = any(FORBIDDEN_CANDIDATE_VALUE in failure for failure in negative_failures)
    if not negative_rejected:
        failures.append("negative sensitive candidate was not rejected by field path")
    if negative_value_logged:
        failures.append("negative sensitive candidate failure leaked fixture value")

    result = {
        "status": "fail" if failures else "pass",
        "baseline_config_path": str(baseline_path.relative_to(ROOT)),
        "candidate_config_path": str(candidate_path.relative_to(ROOT)),
        "baseline_config_hash": sha256(baseline_path) if baseline_path.exists() else None,
        "candidate_config_hash": sha256(candidate_path) if candidate_path.exists() else None,
        "changed_field_paths": paths,
        "changed_field_paths_present": bool(paths),
        "config_diff_review_passed": bool(paths) and not baseline_failures and not candidate_failures,
        "config_diff_review_rejected_sensitive_candidate": negative_rejected,
        "config_diff_review_secret_value_logged": negative_value_logged,
        "config_diff_review_reports_path_only": negative_rejected and not negative_value_logged,
        "config_diff_summary_uses_hashes": True,
        "baseline_config_hash_present": baseline_path.exists(),
        "candidate_config_hash_present": candidate_path.exists(),
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "failures": failures,
    }
    if not result["changed_field_paths_present"]:
        failures.append("config diff review must report changed field paths")
    if not result["config_diff_review_passed"]:
        failures.append("valid config diff review did not pass")
    result["status"] = "fail" if failures else "pass"
    result["failures"] = failures
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
