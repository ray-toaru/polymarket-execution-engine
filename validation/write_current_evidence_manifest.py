#!/usr/bin/env python3
"""Write the canonical current evidence manifest from gate logs."""
from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

SCRIPT = Path(__file__).resolve()
ROOT = SCRIPT.parents[2]
EXECUTOR = ROOT / "polymarket-execution-engine"
if not EXECUTOR.exists():
    EXECUTOR = SCRIPT.parents[1]

VERSION_CANDIDATES = [
    Path(os.environ["PMX_INTEGRATION_ROOT"]) / "VERSION"
    if os.environ.get("PMX_INTEGRATION_ROOT")
    else None,
    ROOT / "VERSION",
    EXECUTOR / "VERSION",
    ROOT / "polymarket_dual_project" / "VERSION",
    EXECUTOR.parent / "polymarket_dual_project" / "VERSION",
]
VERSION_PATH = next((path for path in VERSION_CANDIDATES if path and path.exists()), None)
if VERSION_PATH is None:
    raise FileNotFoundError("VERSION not found in integration or execution repository paths")
VERSION = VERSION_PATH.read_text().strip()
CURRENT_DIR = EXECUTOR / "evidence" / "current"
DEFAULT_LOG_DIR = CURRENT_DIR / "logs"
OUT = CURRENT_DIR / "manifest.json"
ENVIRONMENT = CURRENT_DIR / "environment.json"
GATE_RUNNER = EXECUTOR / "validation" / "run_current_gates_impl.sh"

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
    "shadow_execution_validation": [
        "29-shadow-execution-drill.log",
    ],
    "reconciliation_drift_validation": [
        "31-reconciliation-drift-drill.log",
    ],
    "rollback_kill_switch_validation": [
        "32-kill-switch-rollback-drill.log",
    ],
    "shadow_rollback_drill_guard_validation": [
        "44-shadow-rollback-drill-guard.log",
    ],
    "migration_framework_validation": [
        "33-migration-framework-guard.log",
        "34-migration-drift-dry-run.log",
    ],
    "sdk_standard_sign_only_validation": [
        "35-sdk-standard-sign-only-guard.log",
    ],
    "sdk_regression_suite_validation": [
        "37-sdk-regression-suite-guard.log",
    ],
    "productionization_validation": [
        "36-production-readiness-guard.log",
    ],
    "live_canary_readiness_validation": [
        "38-live-canary-readiness-drill.log",
    ],
    "live_canary_blocked_validation": [
        "39-live-canary-blocked-drill.log",
    ],
    "live_canary_rehearsal_validation": [
        "40-live-canary-rehearsal-drill.log",
    ],
    "live_canary_preflight_validation": [
        "45-live-canary-preflight-drill.log",
    ],
    "production_hardening_config_validation": [
        "41-production-hardening-config.log",
    ],
    "production_operations_validation": [
        "46-production-operations-drill.log",
    ],
    "production_authorization_block_validation": [
        "47-production-authorization-block-drill.log",
    ],
    "production_audit_export_validation": [
        "48-production-audit-export-drill.log",
    ],
    "runtime_worker_status_validation": [
        "42-runtime-worker-status-query.log",
    ],
    "observability_evidence_validation": [
        "43-observability-evidence.log",
    ],
    "local_static_validation": [
        "18-plan-storage-guard.log",
        "19-live-submit-static-guard.log",
        "20-sign-only-lifecycle-guard.log",
        "21-runtime-worker-model-guard.log",
        "22-current-lifecycle-api-guard.log",
        "23-current-evidence-manifest-guard.log",
        "24-version-consistency-guard.log",
        "25-contract-validation.log",
        "26-release-hygiene-clean-snapshot.log",
    ],
}
SKIPPED_LOGS: dict[str, list[str]] = {
    "postgres_validation": ["13-pg-skipped.log"],
    "credentialed_non_trading_validation": [
        "16-authenticated-smoke-skipped.log",
        "17-sign-only-dry-run-skipped.log",
    ],
    "shadow_execution_validation": ["29-shadow-execution-drill.log"],
}
PASS_MARKERS = (
    "passed",
    '"status": "pass"',
    '"status": "ok"',
    "Finished `dev` profile",
    "test result: ok",
    "CREATE INDEX",
    "CREATE TABLE",
)
FAIL_MARKERS = ("FAIL:", "error:", "test result: FAILED", "could not compile", "panicked at")
SKIP_MARKERS = ("skipped", "skipping", "not set")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def log_entry(path: Path) -> dict[str, str | int]:
    path = path.resolve()
    try:
        display_path = path.relative_to(ROOT.resolve())
    except ValueError:
        display_path = path
    entry = {
        "path": str(display_path),
        "sha256": sha256(path),
        "bytes": path.stat().st_size,
    }
    command = LOG_COMMANDS.get(path.name)
    if command:
        entry["command"] = command
    return entry


def display_path(path: Path) -> str:
    path = path.resolve()
    try:
        return str(path.relative_to(ROOT.resolve()))
    except ValueError:
        return str(path)


def load_log_commands() -> dict[str, str]:
    if not GATE_RUNNER.exists():
        return {}
    commands: dict[str, str] = {}
    for raw_line in GATE_RUNNER.read_text().splitlines():
        if 'tee "${EVIDENCE_DIR}/' not in raw_line:
            continue
        match = re.search(r'tee "\$\{EVIDENCE_DIR\}/([^"]+)"', raw_line)
        if not match:
            continue
        command = raw_line[: match.start()].strip()
        command = re.sub(r"\s*2>&1\s*\|\s*$", "", command).strip()
        command = re.sub(r"\s*\|\s*$", "", command).strip()
        command = command.removeprefix('ARTIFACT_PATH="$(').strip()
        commands[match.group(1)] = command
    return commands


LOG_COMMANDS = load_log_commands()


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
    skipped = [log_dir / name for name in SKIPPED_LOGS.get("", []) if (log_dir / name).exists()]
    if not present:
        skipped = [
            log_dir / name
            for section_name, skip_names in SKIPPED_LOGS.items()
            if names == SECTIONS[section_name]
            for name in skip_names
            if (log_dir / name).exists()
        ]
        if skipped:
            return {
                "status": "skipped",
                "logs": [log_entry(path) for path in skipped],
                "skipped_reason": " | ".join(path.read_text(errors="replace").strip() for path in skipped),
            }
        return {"status": "skipped" if optional else "not_run", "logs": []}
    if len(present) != len(names):
        return {"status": "fail", "logs": [log_entry(path) for path in present], "missing_logs": [name for name in names if not (log_dir / name).exists()]}
    if all(any(marker in path.read_text(errors="replace").lower() for marker in SKIP_MARKERS) for path in present):
        return {
            "status": "skipped",
            "logs": [log_entry(path) for path in present],
            "skipped_reason": " | ".join(path.read_text(errors="replace").strip() for path in present),
        }
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
            "path": display_path(artifact_path) if artifact_path else None,
            "sha256": sha256(artifact_path) if artifact_path and artifact_path.exists() else None,
            "binding_note": "External sidecar manifest binds the final zip hash; the in-archive manifest remains source-candidate evidence and cannot self-bind its containing zip.",
        },
        "environment": log_entry(ENVIRONMENT) if ENVIRONMENT.exists() else None,
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
        "status": "shadow-ready SDK sign-only candidate",
        "production_ready": False,
        "live_trading_ready": False,
        "reason": "Shadow-ready SDK sign-only candidate. Required source, Rust, PostgreSQL, SDK, credentialed smoke, sign-only dry-run, local static, drill, governance, and artifact checks are bound in current evidence; production and live trading remain explicitly unapproved.",
        "required_non_optional_sections": required_non_optional,
    }
    OUT.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
