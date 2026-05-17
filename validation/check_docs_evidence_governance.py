#!/usr/bin/env python3
"""Guard current documentation, evidence, and agent-instruction layout."""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXECUTOR = ROOT / "polymarket-execution-engine"
EVIDENCE = EXECUTOR / "evidence"
CURRENT_MANIFEST = EVIDENCE / "current" / "manifest.json"
RELEASE_MANIFEST = EXECUTOR / "release" / "manifest.json"
PACKAGE_SCRIPT = ROOT / "scripts" / "package_release.py"
ARTIFACT_CHECK = ROOT / "scripts" / "check_release_artifact.py"

STALE_ROOT_PATTERNS = [
    re.compile(r"^V0_.*\.md$"),
    re.compile(r"^VALIDATION_V0_.*\.md$"),
    re.compile(r".*_GATE_CONFIRMATION\.md$"),
    re.compile(r"^VALIDATION_CONFIRMATION_REPORT\.md$"),
    re.compile(r"^CONTINUATION_REPORT\.md$"),
    re.compile(r"^ISSUES_CONFIRMED_AND_FIXED\.md$"),
]
VALID_STATUSES = {"pending", "pass", "fail", "skipped", "not_run"}
REQUIRED_SECTIONS = [
    "local_static_validation",
    "rust_workspace_validation",
    "postgres_validation",
    "sdk_adapter_validation",
    "credentialed_non_trading_validation",
]


def fail(message: str) -> int:
    print(f"FAIL: {message}")
    return 1


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def validate_root_docs(failures: list[str]) -> None:
    stale = []
    for path in ROOT.glob("*.md"):
        if any(pattern.match(path.name) for pattern in STALE_ROOT_PATTERNS):
            stale.append(path.name)
    if stale:
        failures.append("stale historical root docs must live in docs/archive: " + ", ".join(sorted(stale)))
    if not (ROOT / "DOC_STATUS.md").exists():
        failures.append("DOC_STATUS.md missing")


def validate_evidence_layout(failures: list[str]) -> None:
    if not CURRENT_MANIFEST.exists():
        failures.append(f"current evidence manifest missing: {CURRENT_MANIFEST.relative_to(ROOT)}")
    for child in EVIDENCE.iterdir():
        if child.name in {"current", "archive"}:
            continue
        failures.append(f"non-canonical evidence entry outside current/archive: {child.relative_to(ROOT)}")
    for path in EVIDENCE.rglob("*.log"):
        rel = path.relative_to(EVIDENCE)
        if not (rel.parts[:2] == ("current", "logs") or rel.parts[:1] == ("archive",)):
            failures.append(f"evidence log outside current/logs or archive: {path.relative_to(ROOT)}")
    root_validation = ROOT / "validation"
    if root_validation.exists():
        for child in root_validation.iterdir():
            if child.is_dir() and child.name.startswith("2026-"):
                failures.append(f"validation dated directory outside archive: {child.relative_to(ROOT)}")


def validate_release_binding(failures: list[str]) -> None:
    if not RELEASE_MANIFEST.exists():
        failures.append("release manifest missing")
        return
    data = json.loads(RELEASE_MANIFEST.read_text())
    binding = data.get("canonical_evidence")
    if not isinstance(binding, dict):
        failures.append("release manifest missing canonical_evidence block")
        return
    expected = "polymarket-execution-engine/evidence/current/manifest.json"
    if binding.get("manifest_path") != expected:
        failures.append(f"release manifest canonical_evidence.manifest_path must be {expected}")
    if binding.get("historical_evidence_policy") != "archive-excluded-from-release-package":
        failures.append("release manifest must state archive-excluded-from-release-package evidence policy")


def validate_current_manifest(failures: list[str]) -> None:
    if not CURRENT_MANIFEST.exists():
        return
    data = json.loads(CURRENT_MANIFEST.read_text())
    if data.get("version") != (ROOT / "VERSION").read_text().strip():
        failures.append("current evidence manifest version must match VERSION")
    if data.get("canonical_evidence_dir") != "polymarket-execution-engine/evidence/current":
        failures.append("current evidence manifest must name canonical evidence dir")
    if data.get("release_decision", {}).get("validated_release") is True:
        non_pass = [section for section in REQUIRED_SECTIONS if data.get(section, {}).get("status") != "pass"]
        if non_pass:
            failures.append(f"validated_release=true with non-pass sections: {non_pass}")
        artifact = data.get("artifact", {})
        if not artifact.get("sha256"):
            failures.append("validated_release=true requires artifact.sha256")
    def validate_log_entries(label: str, logs: object) -> None:
        if not isinstance(logs, list):
            failures.append(f"{label} must be a list")
            return
        for entry in logs:
            if not isinstance(entry, dict):
                failures.append(f"{label} entry must be an object")
                continue
            rel = entry.get("path")
            if not rel:
                failures.append(f"{label} entry missing path")
                continue
            path = ROOT / rel
            if not path.exists():
                failures.append(f"manifest log missing: {rel}")
                continue
            expected_hash = entry.get("sha256")
            if expected_hash and sha256(path) != expected_hash:
                failures.append(f"manifest log hash mismatch: {rel}")

    for section in REQUIRED_SECTIONS:
        block = data.get(section)
        if not isinstance(block, dict):
            failures.append(f"current evidence manifest missing section {section}")
            continue
        if block.get("status") not in VALID_STATUSES:
            failures.append(f"invalid status for {section}: {block.get('status')}")
        validate_log_entries(f"{section}.logs", block.get("logs", []))
    validate_log_entries("additional_logs", data.get("additional_logs", []))



def validate_execution_docs_and_gates(failures: list[str]) -> None:
    docs_dir = EXECUTOR / "docs"
    docs_archive = docs_dir / "archive"
    # docs/archive is expected in the working source tree when historical notes are retained,
    # but release packages intentionally exclude archive directories.
    active_versioned = [
        path.name
        for path in docs_dir.glob("V0_*.md")
        if path.name != "V0_23_SOURCE_CANDIDATE.md"
    ]
    if active_versioned:
        failures.append("stale execution-engine versioned docs must live in docs/archive: " + ", ".join(sorted(active_versioned)))
    if not (docs_dir / "DOC_STATUS.md").exists():
        failures.append("polymarket-execution-engine/docs/DOC_STATUS.md missing")

    validation_dir = EXECUTOR / "validation"
    validation_archive = validation_dir / "archive"
    # validation/archive is also excluded from release packages; only active scripts are checked here.
    allowed_gate_scripts = {"run_current_gates.sh", "run_v0_23_gates.sh"}
    active_old_gates = [path.name for path in validation_dir.glob("run_v0_*_gates.sh") if path.name not in allowed_gate_scripts]
    if active_old_gates:
        failures.append("stale gate scripts must live in validation/archive: " + ", ".join(sorted(active_old_gates)))
    if not (validation_dir / "run_current_gates.sh").exists():
        failures.append("run_current_gates.sh missing")
    if not (validation_dir / "templates" / "evidence_manifest.template.json").exists():
        failures.append("evidence manifest template must live in validation/templates")
    if (EVIDENCE / "v0.23").exists():
        failures.append("evidence/v0.23 must not exist; use evidence/current for canonical evidence and validation/templates for templates")
    sql_todos = sorted(path.relative_to(ROOT).as_posix() for path in validation_dir.rglob("*todo*"))
    if sql_todos:
        failures.append("validation TODO artifacts must be renamed or archived: " + ", ".join(sql_todos))


def validate_agents_guidance(failures: list[str]) -> None:
    required = [
        ROOT / "AGENTS.md",
        ROOT / "hermes-polymarket-control" / "AGENTS.md",
        EXECUTOR / "AGENTS.md",
        EXECUTOR / "crates" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-api" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-authz" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-core" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-gateway" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-policy" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-release" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-runtime" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-service" / "AGENTS.md",
        EXECUTOR / "crates" / "pmx-store" / "AGENTS.md",
        EXECUTOR / "adapters" / "AGENTS.md",
        EXECUTOR / "openapi" / "AGENTS.md",
        EXECUTOR / "migrations" / "AGENTS.md",
        EXECUTOR / "validation" / "AGENTS.md",
    ]
    for path in required:
        if not path.exists():
            failures.append(f"AGENTS.md missing: {path.relative_to(ROOT)}")
            continue
        content = path.read_text()
        if len(content.strip()) < 200:
            failures.append(f"AGENTS.md appears too small to be useful: {path.relative_to(ROOT)}")
    versioned_agents_pattern = re.compile(
        r"(?:\b0\.\d+(?:\.\d+)?\b|\bv0\.\d+\b|\bv0_\d+\b|\bV0_\d+\b|run_v0_\d+_gates\.sh)",
        re.IGNORECASE,
    )
    for agents_path in required:
        if not agents_path.exists():
            continue
        text = agents_path.read_text()
        if versioned_agents_pattern.search(text):
            failures.append(f"AGENTS.md must not contain version-specific release markers: {agents_path.relative_to(ROOT)}")

    root_agents = ROOT / "AGENTS.md"
    if root_agents.exists():
        text = root_agents.read_text()
        for token in ["live submit", "evidence/current", "check_version_consistency.py", "Do not encode the current version"]:
            if token not in text:
                failures.append(f"root AGENTS.md missing required guidance token: {token}")
    hermes_agents = ROOT / "hermes-polymarket-control" / "AGENTS.md"
    if hermes_agents.exists():
        text = hermes_agents.read_text()
        for token in ["must not hold private keys", "must not sign orders", "pytest"]:
            if token not in text:
                failures.append(f"Hermes AGENTS.md missing required guidance token: {token}")
    executor_agents = EXECUTOR / "AGENTS.md"
    if executor_agents.exists():
        text = executor_agents.read_text()
        for token in ["run_current_gates.sh", "cargo check --workspace --locked", "Live submit", "module-level `AGENTS.md`"]:
            if token not in text:
                failures.append(f"execution-engine AGENTS.md missing required guidance token: {token}")

    module_expectations = {
        EXECUTOR / "crates" / "pmx-api" / "AGENTS.md": ["OpenAPI", "service/admin token", "Live submit"],
        EXECUTOR / "crates" / "pmx-authz" / "AGENTS.md": ["fail closed", "empty service/admin tokens"],
        EXECUTOR / "crates" / "pmx-core" / "AGENTS.md": ["deterministic serialization", "sensitive fields"],
        EXECUTOR / "crates" / "pmx-gateway" / "AGENTS.md": ["no remote side effects", "live remote side effects"],
        EXECUTOR / "crates" / "pmx-policy" / "AGENTS.md": ["Runtime `Degraded`", "Loosening"],
        EXECUTOR / "crates" / "pmx-release" / "AGENTS.md": ["validated_release", "external sidecars"],
        EXECUTOR / "crates" / "pmx-runtime" / "AGENTS.md": ["fail closed", "TTL"],
        EXECUTOR / "crates" / "pmx-service" / "AGENTS.md": ["server-authoritative", "client_event_id"],
        EXECUTOR / "crates" / "pmx-store" / "AGENTS.md": ["advisory-lock", "PostgreSQL"],
        EXECUTOR / "adapters" / "AGENTS.md": ["no remote side effects", "env gates"],
        EXECUTOR / "openapi" / "AGENTS.md": ["redacted schemas", "validate_contracts.py"],
        EXECUTOR / "migrations" / "AGENTS.md": ["forward-only", "PostgreSQL validation evidence"],
        EXECUTOR / "validation" / "AGENTS.md": ["run_current_gates.sh", "evidence/current"],
    }
    for path, tokens in module_expectations.items():
        if not path.exists():
            continue
        text = path.read_text()
        for token in tokens:
            if token not in text:
                failures.append(f"{path.relative_to(ROOT)} missing required guidance token: {token}")


def validate_packaging_scripts(failures: list[str]) -> None:
    package_text = PACKAGE_SCRIPT.read_text()
    for token in ["docs/archive", "evidence/archive", "validation/archive", "polymarket-execution-engine/validation/archive", "external_reviews"]:
        if token not in package_text:
            failures.append(f"package_release.py must exclude {token}")
    artifact_text = ARTIFACT_CHECK.read_text()
    for token in ["canonical evidence manifest", "docs/archive", "evidence/archive", "validation/archive"]:
        if token not in artifact_text:
            failures.append(f"check_release_artifact.py missing governance check token: {token}")


def main() -> int:
    failures: list[str] = []
    validate_root_docs(failures)
    validate_evidence_layout(failures)
    validate_release_binding(failures)
    validate_current_manifest(failures)
    validate_execution_docs_and_gates(failures)
    validate_agents_guidance(failures)
    validate_packaging_scripts(failures)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("docs/evidence governance guard passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
