#!/usr/bin/env python3
"""Write the canonical v0.23 current evidence manifest from gate logs."""
from __future__ import annotations

import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXECUTOR = ROOT / "polymarket-execution-engine"
VERSION = (ROOT / "VERSION").read_text().strip()
CURRENT_DIR = EXECUTOR / "evidence" / "current"
DEFAULT_LOG_DIR = CURRENT_DIR / "logs"
OUT = CURRENT_DIR / "manifest.json"

SECTIONS: dict[str, list[str]] = {
    "rust_workspace_validation": [
        "01-cargo-fmt.log",
        "02-cargo-check.log",
        "03-cargo-clippy.log",
        "04-cargo-test-workspace-non-api.log",
        "05-http-fake-e2e.log",
    ],
    "sdk_adapter_validation": [
        "06-sdk-spike-no-features.log",
        "07-sdk-spike-typecheck.log",
        "08-sdk-adapter-fmt.log",
        "09-sdk-adapter-check.log",
        "10-sdk-adapter-clippy.log",
        "11-sdk-adapter-test.log",
        "12-sdk-adapter-typecheck.log",
    ],
    "postgres_validation": [
        "13-pg-migration.log",
        "14-pg-store-tests.log",
        "15-http-postgres-e2e.log",
    ],
    "credentialed_non_trading_validation": [
        "16-authenticated-smoke.log",
        "17-sign-only-dry-run.log",
    ],
    "local_static_validation": [
        "18-plan-storage-guard.log",
        "19-live-submit-static-guard.log",
        "20-sign-only-lifecycle-guard.log",
        "21-runtime-worker-model-guard.log",
        "22-v0-23-lifecycle-api-guard.log",
        "23-v0-23-evidence-manifest-guard.log",
        "24-version-consistency-guard.log",
        "25-contract-validation.log",
        "26-release-hygiene-clean-snapshot.log",
    ],
}
PASS_MARKERS = (
    "passed",
    '"status": "ok"',
    "Finished `dev` profile",
    "test result: ok",
    "CREATE INDEX",
    "CREATE TABLE",
)
FAIL_MARKERS = ("FAIL:", "error:", "test result: FAILED", "could not compile", "panicked at")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def log_entry(path: Path) -> dict[str, str | int]:
    path = path.resolve()
    return {
        "path": str(path.relative_to(ROOT.resolve())),
        "sha256": sha256(path),
        "bytes": path.stat().st_size,
    }


def log_passed(path: Path) -> bool:
    text = path.read_text(errors="replace")
    if any(marker in text for marker in FAIL_MARKERS):
        return False
    if path.stat().st_size == 0:
        # cargo fmt and rustfmt success can produce an empty log.
        return path.name in {"01-cargo-fmt.log", "08-sdk-adapter-fmt.log"}
    return any(marker in text for marker in PASS_MARKERS) or path.name.endswith("-guard.log")


def build_section(log_dir: Path, names: list[str], *, optional: bool = False) -> dict:
    present = [log_dir / name for name in names if (log_dir / name).exists()]
    if not present:
        return {"status": "skipped" if optional else "not_run", "logs": []}
    if len(present) != len(names):
        return {"status": "fail", "logs": [log_entry(path) for path in present], "missing_logs": [name for name in names if not (log_dir / name).exists()]}
    status = "pass" if all(log_passed(path) for path in present) else "fail"
    return {"status": status, "logs": [log_entry(path) for path in present]}


def main(argv: list[str]) -> int:
    log_dir = (Path(argv[1]) if len(argv) > 1 else DEFAULT_LOG_DIR).resolve()
    artifact_path = Path(argv[2]).resolve() if len(argv) > 2 and argv[2] else None
    CURRENT_DIR.mkdir(parents=True, exist_ok=True)
    data = {
        "version": VERSION,
        "artifact_kind": "source_candidate",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "canonical_evidence_dir": "polymarket-execution-engine/evidence/current",
        "provenance": {
            "kind": "generated_from_gate_logs",
            "log_dir": str(log_dir.relative_to(ROOT.resolve())) if log_dir.is_absolute() and ROOT.resolve() in log_dir.parents else str(log_dir),
            "note": "External Rust/SDK/PostgreSQL logs must be regenerated for the exact final artifact before release promotion.",
        },
        "artifact": {
            "name": artifact_path.name if artifact_path else None,
            "path": artifact_path.name if artifact_path else None,
            "sha256": sha256(artifact_path) if artifact_path and artifact_path.exists() else None,
            "binding_note": "External sidecar manifest binds the final zip hash; the in-archive manifest remains source-candidate evidence and cannot self-bind its containing zip.",
        },
    }
    captured_names = set()
    for section, names in SECTIONS.items():
        captured_names.update(names)
        data[section] = build_section(
            log_dir,
            names,
            optional=section == "credentialed_non_trading_validation",
        )
    extra_logs = [path for path in sorted(log_dir.glob("*.log")) if path.name not in captured_names]
    data["additional_logs"] = [log_entry(path) for path in extra_logs]
    required_non_optional = [
        "local_static_validation",
        "rust_workspace_validation",
        "postgres_validation",
        "sdk_adapter_validation",
    ]
    data["release_decision"] = {
        "validated_release": False,
        "reason": "Source candidate. Do not mark validated_release=true until all required evidence sections pass and artifact.sha256 matches the final package.",
        "required_non_optional_sections": required_non_optional,
    }
    OUT.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
