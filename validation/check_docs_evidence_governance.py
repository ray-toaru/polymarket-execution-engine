#!/usr/bin/env python3
"""Guard current documentation, evidence, and agent-instruction layout."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve()
EXECUTOR = SCRIPT.parents[1]
INTEGRATION_ROOT = SCRIPT.parents[2]
VALIDATION_DIR = SCRIPT.parent
if (INTEGRATION_ROOT / "VERSION").exists() and (INTEGRATION_ROOT / "polymarket-execution-engine").exists():
    ROOT = INTEGRATION_ROOT
    EXECUTOR = ROOT / "polymarket-execution-engine"
    INTEGRATION_MODE = True
else:
    ROOT = EXECUTOR
    INTEGRATION_MODE = False
EVIDENCE = EXECUTOR / "evidence"
CURRENT_MANIFEST = EVIDENCE / "current" / "manifest.json"
RELEASE_MANIFEST = EXECUTOR / "release" / "manifest.json"
PACKAGE_SCRIPT = ROOT / "scripts" / "package_release.py"
ARTIFACT_CHECK = ROOT / "scripts" / "check_release_artifact.py"
RELEASE_POLICY = ROOT / "scripts" / "release_policy.py"
SCRIPTS = ROOT / "scripts"
for path in (VALIDATION_DIR, SCRIPTS):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from release_doc_policy import (
    STALE_ROOT_DOC_PATTERNS,
    contains_historical_root_doc_marker,
    contains_release_specific_agents_marker,
)

VALID_STATUSES = {"pending", "pass", "fail", "skipped", "not_run"}
REQUIRED_SECTIONS = [
    "local_static_validation",
    "rust_workspace_validation",
    "postgres_validation",
    "sdk_adapter_validation",
    "credentialed_non_trading_validation",
]
DOC_STATUS_SECTIONS = (
    "postgres_validation",
    "credentialed_non_trading_validation",
    "sdk_standard_sign_only_validation",
    "real_funds_canary_store_truth_cli_validation",
)
CURRENT_STATUS_DOCS = (
    "IMPLEMENTATION_STATUS.md",
    "RELEASE_DECISION.md",
)
SOURCE_PIN_DRIFT_PHRASES = (
    "current source",
    "current checked source",
    "current source binding",
    "current source and evidence",
)
SOURCE_PIN_ALLOWED_CONTEXTS = (
    "reviewed packet",
    "historical",
    "archive",
)
HEX_SHA_RE = re.compile(r"\b[0-9a-f]{40}\b")


def fail(message: str) -> int:
    print(f"FAIL: {message}")
    return 1


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def import_module_from_path(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load module {name} from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_root_docs(failures: list[str]) -> None:
    stale = []
    historical_content = []
    for path in ROOT.glob("*.md"):
        if any(pattern.match(path.name) for pattern in STALE_ROOT_DOC_PATTERNS):
            stale.append(path.name)
        if contains_historical_root_doc_marker(path.read_text(errors="replace")):
            historical_content.append(path.name)
    if stale:
        failures.append("stale historical root docs must live in docs/archive: " + ", ".join(sorted(stale)))
    if historical_content:
        failures.append(
            "historical version root docs must live in docs/archive: "
            + ", ".join(sorted(historical_content))
        )
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
    expected_version = None
    if (ROOT / "VERSION").exists():
        expected_version = (ROOT / "VERSION").read_text().strip()
    elif RELEASE_MANIFEST.exists():
        expected_version = json.loads(RELEASE_MANIFEST.read_text()).get("version")
    if expected_version and data.get("version") != expected_version:
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
            if not path.exists() and not INTEGRATION_MODE and rel.startswith(
                "polymarket-execution-engine/"
            ):
                path = EXECUTOR / rel.removeprefix("polymarket-execution-engine/")
            if not path.exists():
                failures.append(f"manifest log missing: {rel}")
                continue
            expected_hash = entry.get("sha256")
            if Path(rel).name == "30-docs-evidence-governance.log":
                # This guard is normally tee'd into its own evidence log. The
                # final manifest is regenerated after the guard runs, so this
                # invocation cannot safely validate the previous self-log hash.
                continue
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
    if INTEGRATION_MODE:
        docs = {
            name: (ROOT / name).read_text(errors="replace")
            for name in CURRENT_STATUS_DOCS
            if (ROOT / name).exists()
        }
        failures.extend(current_status_binding_failures(data, docs))
        failures.extend(active_source_pin_drift_failures(ROOT))


def current_status_binding_failures(
    manifest: dict,
    docs: dict[str, str],
    *,
    sections: tuple[str, ...] = DOC_STATUS_SECTIONS,
) -> list[str]:
    failures: list[str] = []
    for doc_name, text in docs.items():
        for section in sections:
            match = re.search(
                rf"`{re.escape(section)}=(pending|pass|fail|skipped|not_run)`",
                text,
            )
            if match is None:
                failures.append(
                    f"{doc_name} missing current status binding for {section}"
                )
                continue
            manifest_status = manifest.get(section, {}).get("status")
            documented_status = match.group(1)
            if documented_status != manifest_status:
                failures.append(
                    f"{doc_name} current status for {section} is "
                    f"{documented_status}, manifest is {manifest_status}"
                )
    return failures


def active_source_pin_drift_failures(root: Path) -> list[str]:
    if not INTEGRATION_MODE:
        return []
    try:
        from active_docs import ACTIVE_DOCS
    except Exception as exc:
        return [f"unable to load active docs list: {exc}"]

    failures: list[str] = []
    for doc_name in ACTIVE_DOCS:
        path = root / doc_name
        if not path.exists():
            continue
        lines = path.read_text(errors="replace").splitlines()
        for index, line in enumerate(lines):
            lowered = line.lower()
            if not any(phrase in lowered for phrase in SOURCE_PIN_DRIFT_PHRASES):
                continue
            window = "\n".join(lines[index : index + 8])
            window_lower = window.lower()
            if not HEX_SHA_RE.search(window):
                continue
            if any(context in window_lower for context in SOURCE_PIN_ALLOWED_CONTEXTS):
                continue
            failures.append(
                f"{doc_name}:{index + 1} uses current-source wording with fixed commit pins; "
                "use reviewed-packet or generated snapshot wording"
            )
    return failures



def validate_execution_docs_and_gates(failures: list[str]) -> None:
    docs_dir = EXECUTOR / "docs"
    docs_archive = docs_dir / "archive"
    # docs/archive is expected in the working source tree when historical notes are retained,
    # but release packages intentionally exclude archive directories.
    active_versioned = [
        path.name
        for path in docs_dir.glob("V0_*.md")
    ]
    if active_versioned:
        failures.append("stale execution-engine versioned docs must live in docs/archive: " + ", ".join(sorted(active_versioned)))
    if not (docs_dir / "DOC_STATUS.md").exists():
        failures.append("polymarket-execution-engine/docs/DOC_STATUS.md missing")

    validation_dir = EXECUTOR / "validation"
    validation_archive = validation_dir / "archive"
    # validation/archive is also excluded from release packages; only active scripts are checked here.
    allowed_gate_scripts = {"run_current_gates.sh", "run_current_gates_impl.sh"}
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
    required = []
    if INTEGRATION_MODE:
        required.extend(
            [
                ROOT / "AGENTS.md",
                ROOT / "hermes-polymarket-executor-adapter" / "AGENTS.md",
            ]
        )
    required.extend([
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
    ])
    for path in required:
        if not path.exists():
            failures.append(f"AGENTS.md missing: {path.relative_to(ROOT)}")
            continue
        content = path.read_text()
        if len(content.strip()) < 200:
            failures.append(f"AGENTS.md appears too small to be useful: {path.relative_to(ROOT)}")
    for agents_path in required:
        if not agents_path.exists():
            continue
        text = agents_path.read_text()
        if contains_release_specific_agents_marker(text):
            failures.append(f"AGENTS.md must not contain version-specific release markers: {agents_path.relative_to(ROOT)}")

    root_agents = ROOT / "AGENTS.md"
    if INTEGRATION_MODE and root_agents.exists():
        text = root_agents.read_text()
        for token in ["live submit", "evidence/current", "check_version_consistency.py", "Do not encode the current version"]:
            if token not in text:
                failures.append(f"root AGENTS.md missing required guidance token: {token}")
    hermes_agents = ROOT / "hermes-polymarket-executor-adapter" / "AGENTS.md"
    if INTEGRATION_MODE and hermes_agents.exists():
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
    if not INTEGRATION_MODE:
        return
    if not RELEASE_POLICY.exists():
        failures.append("release_policy.py missing")
        return
    try:
        policy_module = import_module_from_path("pmx_release_policy", RELEASE_POLICY)
    except Exception as exc:
        failures.append(f"unable to load release_policy.py: {exc}")
        return
    package_text = (ROOT / "polymarket-execution-engine" / "validation" / "package_release.py").read_text()
    required_exclusions = {
        "docs/archive": {"docs/archive"},
        "evidence/archive": {
            "evidence/archive",
            "polymarket-execution-engine/evidence/archive",
        },
        "validation/archive": {"validation/archive"},
        "polymarket-execution-engine/validation/archive": {
            "polymarket-execution-engine/validation/archive"
        },
        "external_reviews": {"external_reviews"},
    }
    excluded = getattr(policy_module, "EXCLUDED_PREFIXES", set())
    for token, accepted_values in required_exclusions.items():
        if excluded.isdisjoint(accepted_values):
            failures.append(f"release_policy.py must exclude {token}")
    if "from release_policy import" not in package_text:
        failures.append("package_release.py must import shared release_policy")
    artifact_text = (ROOT / "polymarket-execution-engine" / "validation" / "check_release_artifact.py").read_text()
    if "from release_policy import" not in artifact_text:
        failures.append("check_release_artifact.py must import shared release_policy")
    for token in ["canonical evidence manifest", "validate_dist_index", "release_policy"]:
        if token not in artifact_text:
            failures.append(f"check_release_artifact.py missing governance check token: {token}")


def main() -> int:
    failures: list[str] = []
    if INTEGRATION_MODE:
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
