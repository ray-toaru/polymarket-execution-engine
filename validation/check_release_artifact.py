#!/usr/bin/env python3
from __future__ import annotations

import json
import hashlib
import sys
import zipfile
from pathlib import Path

ENGINE_ROOT = Path(__file__).resolve().parents[1]
ROOT = ENGINE_ROOT.parent
SCRIPT_DIR = Path(__file__).resolve().parent
INTEGRATION_SCRIPT_DIR = ROOT / "scripts"
for path in (SCRIPT_DIR, INTEGRATION_SCRIPT_DIR):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from check_dist_index import validate as validate_dist_index
from package_release import release_source_files, command_output, submodule_records
from release_policy import is_allowed_release_source_path, is_forbidden_release_member
from release_doc_policy import (
    STALE_ROOT_DOC_PATTERNS,
    contains_historical_root_doc_marker,
    contains_release_specific_agents_marker,
)


def forbidden(member: str, expected_root: str | None = None) -> bool:
    return is_forbidden_release_member(member, expected_root=expected_root)


def outside_release_allowlist(member: str, expected_root: str | None = None) -> bool:
    return not is_allowed_release_source_path(member, expected_root=expected_root)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def stale_root_doc(member: str, expected_root: str) -> bool:
    prefix = expected_root + "/"
    if not member.startswith(prefix):
        return False
    rel = member[len(prefix) :]
    if "/" in rel:
        return False
    return any(pattern.match(rel) for pattern in STALE_ROOT_DOC_PATTERNS)


def historical_root_doc_content(member: str, expected_root: str, zf: zipfile.ZipFile) -> bool:
    prefix = expected_root + "/"
    if not member.startswith(prefix):
        return False
    rel = member[len(prefix) :]
    if "/" in rel or not rel.endswith(".md"):
        return False
    return contains_historical_root_doc_marker(zf.read(member).decode(errors="replace"))



def stale_engine_doc(member: str, expected_root: str) -> bool:
    prefix = expected_root + "/polymarket-execution-engine/docs/"
    if not member.startswith(prefix):
        return False
    rel = member[len(prefix) :]
    if "/" in rel:
        return False
    return rel.startswith("V0_") and rel.endswith(".md")


def load_json_object(path: Path) -> dict:
    data = json.loads(path.read_text())
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


def git_head(path: Path) -> str | None:
    return command_output(["git", "-C", str(path), "rev-parse", "HEAD"])


def validate_sidecars(
    zip_path: Path,
    *,
    expected_version: str,
    expected_hash: str,
    expected_git_head: str | None = None,
    expected_submodules: list[dict[str, str]] | None = None,
) -> tuple[list[str], dict | None]:
    failures: list[str] = []
    sidecar = zip_path.with_suffix(zip_path.suffix + ".sha256")
    evidence_sidecar = zip_path.with_suffix(zip_path.suffix + ".evidence.json")
    evidence: dict | None = None

    if not sidecar.exists():
        failures.append(f"SHA-256 sidecar missing: {sidecar}")
    else:
        parts = sidecar.read_text().strip().split()
        if len(parts) < 2:
            failures.append("SHA-256 sidecar must contain '<sha256>  <artifact-name>'")
        else:
            if parts[0] != expected_hash:
                failures.append("SHA-256 sidecar hash does not match artifact")
            if parts[1] != zip_path.name:
                failures.append("SHA-256 sidecar artifact name does not match zip name")

    if not evidence_sidecar.exists():
        failures.append(f"evidence sidecar missing: {evidence_sidecar}")
        return failures, None

    try:
        evidence = load_json_object(evidence_sidecar)
    except ValueError as exc:
        failures.append(str(exc))
        return failures, None

    artifact = evidence.get("artifact", {})
    if artifact.get("name") != zip_path.name:
        failures.append("evidence sidecar artifact.name does not match zip name")
    if artifact.get("sha256") != expected_hash:
        failures.append("evidence sidecar artifact.sha256 does not match artifact")
    if artifact.get("sha256_sidecar") != sidecar.name:
        failures.append("evidence sidecar artifact.sha256_sidecar does not match sidecar")
    source = evidence.get("source", {})
    if source.get("version") != expected_version:
        failures.append("evidence sidecar source.version does not match expected version")
    actual_git_head = source.get("git_head")
    if not actual_git_head:
        failures.append("evidence sidecar source.git_head is missing")
    elif expected_git_head is not None and actual_git_head != expected_git_head:
        failures.append("evidence sidecar source.git_head does not match current workspace HEAD")
    submodules = source.get("submodules")
    if not isinstance(submodules, list) or not submodules:
        failures.append("evidence sidecar source.submodules must be a structured non-empty list")
    else:
        for record in submodules:
            if not isinstance(record, dict):
                failures.append("evidence sidecar source.submodules entries must be objects")
                continue
            for field in ["path", "commit", "checkout_status", "checkout_ref"]:
                if field not in record:
                    failures.append(f"evidence sidecar submodule record missing {field}")
        if expected_submodules is not None and submodules != expected_submodules:
            failures.append("evidence sidecar source.submodules do not match current workspace submodule pins")
    canonical_evidence = evidence.get("canonical_evidence", {})
    if canonical_evidence.get("manifest_path") != "polymarket-execution-engine/evidence/current/manifest.json":
        failures.append("evidence sidecar canonical_evidence.manifest_path is not current manifest")
    if not canonical_evidence.get("archived_manifest_sha256"):
        failures.append("evidence sidecar canonical_evidence.archived_manifest_sha256 is missing")
    if not canonical_evidence.get("workspace_manifest_sha256"):
        failures.append("evidence sidecar canonical_evidence.workspace_manifest_sha256 is missing")
    if not canonical_evidence.get("workspace_manifest_snapshot_path"):
        failures.append("evidence sidecar canonical_evidence.workspace_manifest_snapshot_path is missing")
    if canonical_evidence.get("archived_manifest_binding_kind") != "archive_normalized_current_manifest":
        failures.append("evidence sidecar canonical_evidence.archived_manifest_binding_kind is invalid")
    if canonical_evidence.get("workspace_manifest_binding_kind") != "post_package_workspace_snapshot":
        failures.append("evidence sidecar canonical_evidence.workspace_manifest_binding_kind is invalid")
    return failures, evidence


def validate_archive_members(
    zf: zipfile.ZipFile,
    *,
    expected_root: str,
    expected_version: str,
) -> list[str]:
    failures: list[str] = []
    names = zf.namelist()
    roots = {name.split("/", 1)[0] for name in names if name and "/" in name}
    if roots != {expected_root}:
        failures.append(f"archive root mismatch: got {sorted(roots)}, expected {expected_root}")
    version_name = f"{expected_root}/VERSION"
    if version_name not in names:
        failures.append("VERSION missing from archive")
    else:
        actual_version = zf.read(version_name).decode().strip()
        if actual_version != expected_version:
            failures.append(f"VERSION mismatch: got {actual_version}, expected {expected_version}")
    bad = sorted({name for name in names if forbidden(name, expected_root)})
    if bad:
        failures.append("forbidden archive members: " + ", ".join(bad[:20]))
    outside_allowlist = sorted({name for name in names if outside_release_allowlist(name, expected_root)})
    if outside_allowlist:
        failures.append("archive members outside explicit release allowlist: " + ", ".join(outside_allowlist[:20]))
    stale_docs = sorted({name for name in names if stale_root_doc(name, expected_root)})
    if stale_docs:
        failures.append("stale root docs in archive: " + ", ".join(stale_docs[:20]))
    historical_docs = sorted(
        {name for name in names if historical_root_doc_content(name, expected_root, zf)}
    )
    if historical_docs:
        failures.append("historical root docs in archive: " + ", ".join(historical_docs[:20]))
    stale_engine_docs = sorted({name for name in names if stale_engine_doc(name, expected_root)})
    if stale_engine_docs:
        failures.append("stale execution-engine docs in archive: " + ", ".join(stale_engine_docs[:20]))
    forbidden_evidence_templates = sorted(
        {name for name in names if f"{expected_root}/polymarket-execution-engine/evidence/v" in name}
    )
    if forbidden_evidence_templates:
        failures.append(
            "non-canonical evidence version directory in archive: "
            + ", ".join(forbidden_evidence_templates[:20])
        )
    return failures


def validate_workspace_source_coverage(
    zf: zipfile.ZipFile,
    *,
    expected_root: str,
) -> list[str]:
    failures: list[str] = []
    names = {
        name
        for name in zf.namelist()
        if name
        and not name.endswith("/")
        and name != f"{expected_root}/"
    }
    expected = {
        f"{expected_root}/{path.relative_to(ROOT).as_posix()}"
        for path in release_source_files()
    }
    missing = sorted(expected - names)
    extra = sorted(names - expected)
    if missing:
        failures.append("archive is missing tracked release source files: " + ", ".join(missing[:20]))
    if extra:
        failures.append("archive contains files outside tracked release source set: " + ", ".join(extra[:20]))
    return failures


def validate_shebang_modes(zf: zipfile.ZipFile) -> list[str]:
    failures: list[str] = []
    bad_shebang_modes = []
    for info in zf.infolist():
        if info.is_dir():
            continue
        data = zf.read(info.filename)
        if not data.startswith(b"#!"):
            continue
        mode = (info.external_attr >> 16) & 0o777
        if mode != 0o755:
            bad_shebang_modes.append(f"{info.filename} mode={oct(mode)}")
    if bad_shebang_modes:
        failures.append("shebang scripts must be executable in archive: " + ", ".join(bad_shebang_modes[:20]))
    return failures


def required_agents(expected_root: str) -> list[str]:
    return [
        f"{expected_root}/AGENTS.md",
        f"{expected_root}/hermes-polymarket-executor-adapter/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-api/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-authz/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-core/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-gateway/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-policy/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-release/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-runtime/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-service/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/crates/pmx-store/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/adapters/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/openapi/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/migrations/AGENTS.md",
        f"{expected_root}/polymarket-execution-engine/validation/AGENTS.md",
    ]


def validate_agents_in_archive(zf: zipfile.ZipFile, *, expected_root: str) -> list[str]:
    failures: list[str] = []
    names = set(zf.namelist())
    required = required_agents(expected_root)
    missing_agents = [name for name in required if name not in names]
    if missing_agents:
        failures.append("required AGENTS.md files missing from archive: " + ", ".join(missing_agents))
    for name in required:
        if name not in names:
            continue
        content = zf.read(name).decode()
        if contains_release_specific_agents_marker(content):
            failures.append(f"AGENTS.md contains version-specific release markers: {name}")
    return failures


def validate_manifest_bindings(
    zf: zipfile.ZipFile,
    *,
    expected_root: str,
    expected_version: str,
    expected_hash: str,
    evidence: dict | None,
) -> list[str]:
    failures: list[str] = []
    names = set(zf.namelist())
    current_manifest = f"{expected_root}/polymarket-execution-engine/evidence/current/manifest.json"
    if current_manifest not in names:
        failures.append("canonical evidence manifest missing from archive")
    else:
        manifest_bytes = zf.read(current_manifest)
        data = json.loads(manifest_bytes.decode())
        if data.get("version") != expected_version:
            failures.append("canonical evidence manifest version mismatch")
        if data.get("canonical_evidence_dir") != "polymarket-execution-engine/evidence/current":
            failures.append("canonical evidence manifest has bad canonical_evidence_dir")
        if data.get("release_decision", {}).get("validated_release") is True and not data.get("artifact", {}).get("sha256"):
            failures.append("validated evidence manifest must include artifact sha256")
        external_artifact = data.get("external_artifact_sidecar", {})
        if isinstance(external_artifact, dict):
            embedded_zip_hash = external_artifact.get("sha256")
            if embedded_zip_hash not in (None, expected_hash):
                failures.append(
                    "canonical evidence manifest carries a stale external_artifact_sidecar.sha256"
                )
        if evidence is not None:
            canonical = evidence.get("canonical_evidence", {})
            archive_manifest_sha = hashlib.sha256(manifest_bytes).hexdigest()
            if canonical.get("archived_manifest_sha256") != archive_manifest_sha:
                failures.append("evidence sidecar archived_manifest_sha256 does not match archived manifest")
    release_manifest = f"{expected_root}/polymarket-execution-engine/release/manifest.json"
    if release_manifest not in names:
        failures.append("release manifest missing")
    else:
        data = json.loads(zf.read(release_manifest).decode())
        binding = data.get("canonical_evidence", {})
        if binding.get("manifest_path") != "polymarket-execution-engine/evidence/current/manifest.json":
            failures.append("release manifest does not bind canonical evidence manifest")
    return failures

def main() -> int:
    if len(sys.argv) != 3:
        print("usage: check_release_artifact.py <zip> <expected-version>", file=sys.stderr)
        return 2
    zip_path = Path(sys.argv[1])
    expected_version = sys.argv[2].strip()
    failures: list[str] = []
    failures.extend(validate_dist_index(zip_path.parent, expected_version))
    expected_root = f"polymarket_execution_suite_v{expected_version.replace('.', '_')}"
    expected_hash = sha256(zip_path)
    expected_git = git_head(Path(__file__).resolve().parents[1])
    expected_subs = submodule_records()
    sidecar_failures, evidence = validate_sidecars(
        zip_path,
        expected_version=expected_version,
        expected_hash=expected_hash,
        expected_git_head=expected_git,
        expected_submodules=expected_subs,
    )
    failures.extend(sidecar_failures)
    with zipfile.ZipFile(zip_path) as zf:
        failures.extend(
            validate_archive_members(
                zf,
                expected_root=expected_root,
                expected_version=expected_version,
            )
        )
        failures.extend(
            validate_workspace_source_coverage(
                zf,
                expected_root=expected_root,
            )
        )
        failures.extend(validate_shebang_modes(zf))
        failures.extend(validate_agents_in_archive(zf, expected_root=expected_root))
        failures.extend(
            validate_manifest_bindings(
                zf,
                expected_root=expected_root,
                expected_version=expected_version,
                expected_hash=expected_hash,
                evidence=evidence,
            )
        )
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"release artifact passed root={expected_root} version={expected_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
ENGINE_ROOT = Path(__file__).resolve().parents[1]
ROOT = ENGINE_ROOT.parent
