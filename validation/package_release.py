#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any

ENGINE_ROOT = Path(__file__).resolve().parents[1]
ROOT = ENGINE_ROOT.parent
SCRIPT_DIR = Path(__file__).resolve().parent
INTEGRATION_SCRIPT_DIR = ROOT / "scripts"
for path in (SCRIPT_DIR, INTEGRATION_SCRIPT_DIR):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from release_policy import is_allowed_release_source_path, is_forbidden_release_member

VERSION = (ROOT / "VERSION").read_text().strip()
ARCHIVE_ROOT = f"polymarket_execution_suite_v{VERSION.replace('.', '_')}"
DIST = ROOT / "dist"
OUT = DIST / f"polymarket-execution-suite-v{VERSION}.zip"
DETERMINISTIC_MTIME = (1980, 1, 1, 0, 0, 0)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def command_output(command: list[str]) -> str | None:
    return require_command_output(command)


def optional_command_output(command: list[str]) -> str | None:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        return None
    return completed.stdout.rstrip("\n")


def command_output_bytes(command: list[str], *, cwd: Path = ROOT) -> bytes | None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        return None
    return completed.stdout


def require_command_output(command: list[str], *, cwd: Path = ROOT) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.strip()
        detail = f": {stderr}" if stderr else ""
        raise SystemExit(
            f"command failed ({completed.returncode}) {' '.join(command)}{detail}"
        )
    return completed.stdout.rstrip("\n")


def require_command_output_bytes(command: list[str], *, cwd: Path = ROOT) -> bytes:
    completed = subprocess.run(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.decode(errors="replace").strip()
        detail = f": {stderr}" if stderr else ""
        raise SystemExit(
            f"command failed ({completed.returncode}) {' '.join(command)}{detail}"
        )
    return completed.stdout


def git_branch(path: Path = ROOT) -> str | None:
    return optional_command_output(["git", "-C", str(path), "branch", "--show-current"])


def submodule_records() -> list[dict[str, str]]:
    raw = command_output(["git", "submodule", "status"]) or ""
    records: list[dict[str, str]] = []
    for raw_line in raw.splitlines():
        if not raw_line.strip():
            continue
        status = raw_line[0]
        rest = raw_line[1:].strip() if status in {" ", "+", "-", "U"} else raw_line.strip()
        parts = rest.split()
        if len(parts) < 2:
            continue
        commit = parts[0]
        path = parts[1]
        ref = " ".join(parts[2:]).strip()
        records.append(
            {
                "path": path,
                "commit": commit,
                "checkout_status": status if status != " " else "clean",
                "checkout_ref": ref,
            }
        )
    return records


def tracked_git_files(repo_root: Path) -> list[Path]:
    raw = require_command_output_bytes(["git", "ls-files", "-z"], cwd=repo_root)
    paths = []
    for item in raw.split(b"\0"):
        if not item:
            continue
        try:
            rel = Path(item.decode("utf-8"))
        except UnicodeDecodeError as exc:
            raise SystemExit(
                f"git ls-files emitted non-utf8 path bytes for release packaging: {repo_root}: {exc}"
            ) from exc
        paths.append(repo_root / rel)
    return paths


def git_status_lines(repo_root: Path) -> list[str]:
    raw = require_command_output(["git", "-C", str(repo_root), "status", "--short"])
    return [line for line in raw.splitlines() if line.strip()]


def ensure_clean_release_submodules() -> None:
    dirty: list[str] = []
    for record in submodule_records():
        path = record["path"]
        if record["checkout_status"] != "clean":
            dirty.append(
                f"{path}: checkout_status={record['checkout_status']} checkout_ref={record['checkout_ref']}"
            )
            continue
        submodule_root = ROOT / path
        status_lines = git_status_lines(submodule_root)
        if status_lines:
            dirty.append(f"{path}: dirty worktree ({status_lines[0]})")
    if dirty:
        raise SystemExit(
            "release packaging requires clean pinned submodules: " + "; ".join(dirty)
        )


def release_source_files() -> list[Path]:
    seen: set[Path] = set()
    files: list[Path] = []
    for path in tracked_git_files(ROOT):
        if not path.is_file():
            continue
        if allowed(path) and path not in seen:
            seen.add(path)
            files.append(path)
    for record in submodule_records():
        submodule_root = ROOT / record["path"]
        for path in tracked_git_files(submodule_root):
            if not path.is_file():
                continue
            if allowed(path) and path not in seen:
                seen.add(path)
                files.append(path)
    return sorted(files)


def allowed(path: Path) -> bool:
    rel = path.relative_to(ROOT)
    return is_allowed_release_source_path(rel) and not is_forbidden_release_member(rel)


def executable_in_archive(path: Path) -> bool:
    try:
        first = path.open("rb").read(2)
    except OSError:
        return False
    return first == b"#!"


def archive_bytes(path: Path) -> bytes:
    rel = path.relative_to(ROOT).as_posix()
    data = path.read_bytes()
    if rel == "polymarket-execution-engine/evidence/current/manifest.json":
        manifest = json.loads(data.decode())
        manifest["generated_at"] = "archive-normalized"
        manifest["archive_normalization"] = {
            "reason": (
                "The workspace manifest records the post-package artifact hash and generation "
                "timestamp. The copy embedded in the artifact normalizes volatile fields so the "
                "archive can be deterministically bound by external sidecars without self-reference."
            ),
            "normalized_fields": [
                "generated_at",
                "external_artifact_sidecar.sha256",
            ],
        }
        external = manifest.get("external_artifact_sidecar")
        if isinstance(external, dict):
            external["sha256"] = None
            external["binding_note"] = (
                "Archived manifest copy: external artifact hash is normalized to null "
                "to avoid archive self-reference. Use the external .zip.sha256 and "
                ".zip.evidence.json sidecars, or the post-package workspace manifest, "
                "for final artifact binding."
            )
        data = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    return data


def write_deterministic(zf: ZipFile, path: Path) -> None:
    archive_name = str(Path(ARCHIVE_ROOT) / path.relative_to(ROOT))
    info = ZipInfo(archive_name, DETERMINISTIC_MTIME)
    info.compress_type = ZIP_DEFLATED
    info.create_system = 3
    mode = 0o755 if executable_in_archive(path) else 0o644
    info.external_attr = (mode & 0xFFFF) << 16
    zf.writestr(info, archive_bytes(path))


def build_release_zip() -> None:
    if OUT.exists():
        OUT.unlink()
    with ZipFile(OUT, "w", ZIP_DEFLATED) as zf:
        for path in release_source_files():
            write_deterministic(zf, path)


def load_json_if_exists(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    data = json.loads(path.read_text())
    return data if isinstance(data, dict) else None


def is_reviewed_go_material(
    name: str,
    *,
    child_names: set[str] | None = None,
    dir_path: Path | None = None,
) -> bool:
    child_names = child_names or set()
    if {"review.json", "release-decision.json", "approval.json", "candidate-market.json", "runtime-truth.json"}.issubset(child_names):
        return True
    if dir_path is not None:
        review = load_json_if_exists(dir_path / "review.json")
        decision = load_json_if_exists(dir_path / "release-decision.json")
        if isinstance(review, dict) and str(review.get("status", "")).startswith("reviewed_go_"):
            return True
        if isinstance(decision, dict) and decision.get("status") == "reviewed_go":
            return True
    return name.startswith("pmx-canary-reviewed-go-") or (
        name.startswith("pmx-") and "-reviewed-go-" in name
    )


def classify_dist_entry(
    name: str,
    *,
    is_dir: bool,
    child_names: set[str] | None = None,
    dir_path: Path | None = None,
) -> dict[str, object]:
    child_names = child_names or set()
    status = "local_review_material_not_release_artifact"
    approval_reuse_allowed = False
    remote_side_effects_authorized = False
    if is_reviewed_go_material(name, child_names=child_names, dir_path=dir_path):
        if "closeout.json" in child_names or "CLOSEOUT.md" in child_names:
            status = "consumed_closed"
        elif any(child.startswith("approval-consumed") for child in child_names):
            status = "consumed_not_closed"
        else:
            status = "reviewed_go_local_material_not_current_approval"
    elif name.startswith("pmx-canary-review-") and "no-go" in name:
        status = "current_no_go_review_material"
    elif name.startswith("pmx-canary-review-"):
        status = "review_material_not_release_artifact"
    elif name.startswith("pmx-canary-"):
        status = "historical_or_local_canary_material"
    return {
        "path": name,
        "kind": "directory" if is_dir else "file",
        "status": status,
        "approval_reuse_allowed": approval_reuse_allowed,
        "remote_side_effects_authorized": remote_side_effects_authorized,
    }


ARCHIVED_MANIFEST_BINDING_KIND = "archive_normalized_current_manifest"
WORKSPACE_MANIFEST_BINDING_KIND = "post_package_workspace_snapshot"


def write_dist_index(
    artifact_sha256: str,
    archived_manifest_sha256: str | None,
    workspace_manifest_sha256: str | None,
    workspace_manifest_snapshot_path: str | None,
) -> None:
    DIST.mkdir(parents=True, exist_ok=True)
    current_release_files = {OUT.name, OUT.with_suffix(OUT.suffix + ".sha256").name, OUT.with_suffix(OUT.suffix + ".evidence.json").name}
    local_material = []
    for path in sorted(DIST.iterdir()):
        if path.name in current_release_files or path.name in {"INDEX.json", "README.md"}:
            continue
        child_names = {child.name for child in path.iterdir()} if path.is_dir() else set()
        local_material.append(
            classify_dist_entry(
                path.name,
                is_dir=path.is_dir(),
                child_names=child_names,
                dir_path=path if path.is_dir() else None,
            )
        )
    index = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "version": VERSION,
        "current_release_artifact": {
            "path": OUT.name,
            "sha256": artifact_sha256,
            "sha256_sidecar": OUT.with_suffix(OUT.suffix + ".sha256").name,
            "evidence_sidecar": OUT.with_suffix(OUT.suffix + ".evidence.json").name,
            "artifact_class": "production_live_candidate_non_live_by_default",
            "validated_release": False,
            "production_ready": False,
            "live_trading_ready": False,
        },
        "canonical_evidence": {
            "path": "polymarket-execution-engine/evidence/current/manifest.json",
            "archived_manifest_sha256": archived_manifest_sha256,
            "workspace_manifest_sha256": workspace_manifest_sha256,
            "workspace_manifest_snapshot_path": workspace_manifest_snapshot_path,
            "archived_manifest_binding_kind": ARCHIVED_MANIFEST_BINDING_KIND,
            "workspace_manifest_binding_kind": WORKSPACE_MANIFEST_BINDING_KIND,
        },
        "local_material": local_material,
        "operator_warning": (
            "Only current_release_artifact and its sidecars form the source release artifact. "
            "Other dist entries are local review material and must not be treated as current approval."
        ),
    }
    (DIST / "INDEX.json").write_text(json.dumps(index, indent=2, sort_keys=True) + "\n")
    (DIST / "README.md").write_text(
        "\n".join(
            [
                f"# Polymarket Execution Suite dist index v{VERSION}",
                "",
                "Current source artifact:",
                "",
                f"- `{OUT.name}`",
                f"- SHA-256: `{artifact_sha256}`",
                "- Status: production-live-candidate, non-live",
                "- Not production-ready; not live-trading-ready; not a `go` approval",
                "",
                "`INDEX.json` is the machine-readable index. Any other files or directories in",
                "`dist/` are local review material unless listed as the current release artifact.",
                "",
            ]
        )
    )


def workspace_manifest_snapshot_bytes(evidence_manifest: Path, artifact_sha256: str) -> bytes | None:
    if not evidence_manifest.exists():
        return None
    data = json.loads(evidence_manifest.read_text())
    external = data.setdefault("external_artifact_sidecar", {})
    if isinstance(external, dict):
        external.update(
            {
                "name": OUT.name,
                "path": f"dist/{OUT.name}",
                "sha256": artifact_sha256,
                "sha256_sidecar": f"{OUT.name}.sha256",
                "evidence_sidecar": f"{OUT.name}.evidence.json",
                "binding_note": (
                    "Current workspace evidence binds the final release artifact here. "
                    "When this manifest is archived inside the artifact, package_release.py "
                    "normalizes this volatile hash to null to avoid archive self-reference."
                ),
            }
        )
    data.setdefault("artifact", {}).update(
        {
            "name": None,
            "path": None,
            "sha256": None,
            "binding_note": (
                "The canonical manifest does not self-bind a containing zip. "
                "Release artifacts are bound by external .zip.sha256 and .zip.evidence.json sidecars."
            ),
        }
    )
    return (json.dumps(data, indent=2, sort_keys=True) + "\n").encode()


def write_workspace_manifest_snapshot(
    *,
    evidence_manifest: Path,
    artifact_sha256: str,
    snapshot_path: Path,
) -> str | None:
    data = workspace_manifest_snapshot_bytes(evidence_manifest, artifact_sha256)
    if data is None:
        return None
    snapshot_path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def archived_manifest_sha256(evidence_manifest: Path) -> str | None:
    if not evidence_manifest.exists():
        return None
    return hashlib.sha256(archive_bytes(evidence_manifest)).hexdigest()


def contract_validation_report_metadata() -> dict[str, str] | None:
    report = (
        ROOT
        / "polymarket-execution-engine"
        / "evidence"
        / "current"
        / "logs"
        / "25-contract-validation.report.json"
    )
    if not report.exists():
        return None
    return {
        "path": str(report.relative_to(ROOT)),
        "sha256": sha256(report),
    }


def main() -> int:
    if not VERSION:
        print("VERSION file is empty", file=sys.stderr)
        return 1
    ensure_clean_release_submodules()
    DIST.mkdir(exist_ok=True)
    evidence_manifest = ROOT / "polymarket-execution-engine" / "evidence" / "current" / "manifest.json"
    sidecar = OUT.with_suffix(OUT.suffix + ".sha256")

    # The archived manifest copy is normalized and does not depend on the outer
    # artifact hash. Build the zip once, derive its final hash, then bind that
    # hash into a dist-local workspace manifest snapshot without mutating the
    # canonical current-evidence manifest.
    build_release_zip()
    artifact_sha256 = sha256(OUT)
    workspace_snapshot = DIST / f"polymarket-execution-suite-v{VERSION}.workspace-manifest.json"
    workspace_manifest_sha256 = write_workspace_manifest_snapshot(
        evidence_manifest=evidence_manifest,
        artifact_sha256=artifact_sha256,
        snapshot_path=workspace_snapshot,
    )
    archived_manifest = archived_manifest_sha256(evidence_manifest)
    sidecar.write_text(f"{artifact_sha256}  {OUT.name}\n")
    evidence_sidecar = OUT.with_suffix(OUT.suffix + ".evidence.json")
    evidence_sidecar.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "generated_at": datetime.now(timezone.utc).isoformat(),
                "artifact": {
                    "name": OUT.name,
                    "sha256": artifact_sha256,
                    "sha256_sidecar": sidecar.name,
                },
                "source": {
                    "version": VERSION,
                    "git_head": command_output(["git", "rev-parse", "HEAD"]),
                    "git_branch": git_branch(),
                    "submodules": submodule_records(),
                    "submodule_status_raw": command_output(["git", "submodule", "status"]),
                },
                "canonical_evidence": {
                    "manifest_path": "polymarket-execution-engine/evidence/current/manifest.json",
                    "archived_manifest_sha256": archived_manifest,
                    "workspace_manifest_sha256": workspace_manifest_sha256,
                    "workspace_manifest_snapshot_path": workspace_snapshot.name,
                    "archived_manifest_binding_kind": ARCHIVED_MANIFEST_BINDING_KIND,
                    "workspace_manifest_binding_kind": WORKSPACE_MANIFEST_BINDING_KIND,
                    "contract_validation_report": contract_validation_report_metadata(),
                },
                "release_decision_path": "RELEASE_DECISION.md",
                "note": "This external sidecar binds the final zip hash; files inside the zip do not self-assert the final containing-archive hash.",
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    write_dist_index(
        artifact_sha256,
        archived_manifest,
        workspace_manifest_sha256,
        workspace_snapshot.name if workspace_manifest_sha256 else None,
    )
    print(OUT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
