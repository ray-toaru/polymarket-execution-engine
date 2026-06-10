#!/usr/bin/env python3
"""Guard current evidence manifests against accidental release overclaiming.

The template lives under validation/templates so evidence/current remains the single
canonical evidence location.
"""
from __future__ import annotations

import json
import hashlib
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "validation" / "templates" / "evidence_manifest.template.json"
CURRENT = ROOT / "evidence" / "current" / "manifest.json"
VERSION_PATHS = [
    ROOT.parent / "VERSION",
    ROOT / "VERSION",
]
REQUIRED_SECTIONS = [
    "local_static_validation",
    "rust_workspace_validation",
    "postgres_validation",
    "sdk_adapter_validation",
    "credentialed_non_trading_validation",
]
VALID_STATUSES = {"pending", "pass", "fail", "skipped", "not_run"}
TEST_LOG_RULES = {
    "16-authenticated-smoke.log": {
        "min_passed": 1,
        "required_token": "authenticated_non_trading_smoke_executes_when_enabled",
        "forbidden_token": "skipping",
    },
    "17-sign-only-dry-run.log": {
        "min_passed": 1,
        "required_token": "sign_only_dry_run_executes_when_enabled",
        "forbidden_token": "skipping sign-only dry-run test",
    },
    "14-pg-store-tests.log": {
        "min_passed": 23,
        "required_token": "postgres::postgres_tests::",
        "forbidden_token": "PMX_TEST_DATABASE_URL not set",
    },
}
JSON_LOG_RULES = {
    "72-real-funds-canary-store-truth-cli-preflight.log": {
        "status": "pass",
        "preflight_ready": True,
        "posted": False,
        "remote_side_effects": False,
        "raw_signed_order_exposed": False,
        "runtime_truth_source": "postgres",
        "selected_market_id_hash_present": True,
        "selected_token_id_hash_present": True,
    },
}


def fail(message: str) -> int:
    print(f"FAIL: {message}")
    return 1


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def resolve_manifest_path(rel: str) -> Path:
    path = Path(rel)
    if path.is_absolute():
        return path
    if rel.startswith("polymarket-execution-engine/"):
        return ROOT / rel.removeprefix("polymarket-execution-engine/")
    return ROOT.parent / path


def expected_version() -> str:
    for path in VERSION_PATHS:
        if path.exists():
            return path.read_text().strip()
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    return cargo["workspace"]["package"]["version"]


def validate(path: Path, *, allow_missing_semantic_logs: bool = False) -> int:
    data = json.loads(path.read_text())
    expected = expected_version()
    if data.get("version") != expected:
        return fail(f"manifest version must be {expected}")
    if data.get("artifact_kind") not in {"source_candidate", "validated_release"}:
        return fail("artifact_kind must be source_candidate or validated_release")
    if data.get("canonical_evidence_dir") != "polymarket-execution-engine/evidence/current":
        return fail("canonical_evidence_dir must point at evidence/current")
    artifact = data.get("artifact")
    if not isinstance(artifact, dict):
        return fail("missing artifact block")
    external_artifact = data.get("external_artifact_sidecar")
    if not isinstance(external_artifact, dict):
        return fail("missing external_artifact_sidecar block")
    external_path = external_artifact.get("path")
    external_sha = external_artifact.get("sha256")
    if external_sha is not None:
        if not isinstance(external_sha, str) or not re.fullmatch(r"[0-9a-f]{64}", external_sha):
            return fail("external_artifact_sidecar.sha256 must be lowercase sha256 hex when present")
        if not isinstance(external_path, str) or not external_path:
            return fail("external_artifact_sidecar.path is required when sha256 is present")
        artifact_path = resolve_manifest_path(external_path)
        if not artifact_path.exists():
            return fail(f"external artifact not found: {external_path}")
        if sha256(artifact_path) != external_sha:
            return fail("external_artifact_sidecar.sha256 does not match artifact file")
    for section in REQUIRED_SECTIONS:
        block = data.get(section)
        if not isinstance(block, dict):
            return fail(f"missing evidence section: {section}")
        status = block.get("status")
        if status not in VALID_STATUSES:
            return fail(f"invalid status for {section}: {status}")
        required_logs = block.get("required_logs")
        logs = block.get("logs")
        if required_logs is not None and (not isinstance(required_logs, list) or not all(isinstance(item, str) and item for item in required_logs)):
            return fail(f"{section}.required_logs must be a non-empty string list when present")
        if logs is not None and not isinstance(logs, list):
            return fail(f"{section}.logs must be a list when present")
    decision = data.get("release_decision")
    if not isinstance(decision, dict):
        return fail("missing release_decision")
    if decision.get("validated_release") is True:
        non_pass = [section for section in REQUIRED_SECTIONS if data[section].get("status") != "pass"]
        if non_pass:
            return fail(f"validated_release=true with non-pass evidence sections: {non_pass}")
        if data.get("artifact_kind") != "validated_release":
            return fail("validated_release=true requires artifact_kind=validated_release")
        if not artifact.get("sha256"):
            return fail("validated_release=true requires artifact.sha256")
    for section in REQUIRED_SECTIONS:
        block = data.get(section, {})
        if not isinstance(block, dict):
            continue
        if block.get("status") == "skipped":
            continue
        for entry in block.get("logs", []) or []:
            if not isinstance(entry, dict):
                continue
            rel = entry.get("path")
            if not isinstance(rel, str):
                continue
            log_path = ROOT.parent / rel
            if not log_path.exists() and rel.startswith("polymarket-execution-engine/"):
                log_path = ROOT / rel.removeprefix("polymarket-execution-engine/")
            rc = validate_test_log_semantics(
                log_path,
                allow_missing=allow_missing_semantic_logs,
            )
            if rc:
                return fail(rc)
            rc = validate_json_log_semantics(
                log_path,
                allow_missing=allow_missing_semantic_logs,
            )
            if rc:
                return fail(rc)
    return 0


def validate_test_log_semantics(path: Path, *, allow_missing: bool = False) -> str | None:
    rule = TEST_LOG_RULES.get(path.name)
    if not rule:
        return None
    if not path.exists():
        if allow_missing:
            return None
        return f"test log not found for semantic check: {path}"
    text = path.read_text(errors="replace")
    if "running 0 tests" in text:
        return f"{path.name} must not report running 0 tests"
    summary = re.search(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
        r"(\d+) measured; (\d+) filtered out;",
        text,
    )
    if not summary:
        return f"{path.name} missing cargo test summary"
    passed = int(summary.group(1))
    failed = int(summary.group(2))
    min_passed = int(rule["min_passed"])
    required_token = str(rule["required_token"])
    forbidden_token = str(rule.get("forbidden_token", ""))
    if passed < min_passed:
        return f"{path.name} passed {passed} tests, expected at least {min_passed}"
    if failed != 0:
        return f"{path.name} must have failed=0; got failed={failed}"
    if required_token not in text:
        return f"{path.name} missing expected test module token {required_token}"
    if forbidden_token and forbidden_token in text:
        return f"{path.name} contains forbidden skip token {forbidden_token}"
    return None


def validate_json_log_semantics(path: Path, *, allow_missing: bool = False) -> str | None:
    rule = JSON_LOG_RULES.get(path.name)
    if not rule:
        return None
    if not path.exists():
        if allow_missing:
            return None
        return f"JSON evidence log not found for semantic check: {path}"
    try:
        data = json.loads(path.read_text(errors="replace"))
    except json.JSONDecodeError as exc:
        return f"{path.name} is not valid JSON: {exc}"
    for key, expected in rule.items():
        if data.get(key) != expected:
            return f"{path.name} has unexpected {key}: {data.get(key)!r}; expected {expected!r}"
    return None


def main(argv: list[str]) -> int:
    if len(argv) > 1:
        paths = [(Path(arg), False) for arg in argv[1:]]
    else:
        paths = [(TEMPLATE, False)]
        # During a version-promotion gate, evidence/current can still contain the
        # previous manifest until write_current_evidence_manifest.py regenerates it.
        # The full gate validates the regenerated current manifest later via the
        # docs/evidence governance guard.
        if CURRENT.exists():
            current = json.loads(CURRENT.read_text())
            if current.get("version") == expected_version():
                paths.append((CURRENT, True))
    for path, allow_missing_semantic_logs in paths:
        if not path.exists():
            return fail(f"manifest not found: {path}")
        rc = validate(path, allow_missing_semantic_logs=allow_missing_semantic_logs)
        if rc != 0:
            return rc
    print(f"v{expected_version()} evidence manifest guard passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
