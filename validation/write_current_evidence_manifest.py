#!/usr/bin/env python3
"""Write the canonical current evidence manifest from gate logs."""
from __future__ import annotations

import hashlib
import json
import os
import re
import sys
import tomllib
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
    ROOT / "polymarket_execution_suite" / "VERSION",
    EXECUTOR.parent / "polymarket_execution_suite" / "VERSION",
]
VERSION_PATH = next((path for path in VERSION_CANDIDATES if path and path.exists()), None)
if VERSION_PATH is not None:
    VERSION = VERSION_PATH.read_text().strip()
else:
    cargo = tomllib.loads((EXECUTOR / "Cargo.toml").read_text())
    VERSION = cargo["workspace"]["package"]["version"]
CURRENT_DIR = EXECUTOR / "evidence" / "current"
DEFAULT_LOG_DIR = CURRENT_DIR / "logs"
OUT = CURRENT_DIR / "manifest.json"
ENVIRONMENT = CURRENT_DIR / "environment.json"
GATE_RUNNER = EXECUTOR / "validation" / "run_current_gates_impl.sh"
CONTRACT_VALIDATION_REPORT = DEFAULT_LOG_DIR / "25-contract-validation.report.json"

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
    "production_dependency_breakage_validation": [
        "49-production-dependency-breakage-drill.log",
    ],
    "production_deployment_preflight_validation": [
        "50-production-deployment-preflight-drill.log",
    ],
    "production_secret_custody_validation": [
        "51-production-secret-custody-drill.log",
    ],
    "production_monitoring_slo_validation": [
        "52-production-monitoring-slo-drill.log",
    ],
    "production_incident_response_validation": [
        "53-production-incident-response-drill.log",
    ],
    "production_rollback_downgrade_validation": [
        "54-production-rollback-downgrade-drill.log",
    ],
    "production_risk_limits_validation": [
        "55-production-risk-limits-drill.log",
    ],
    "production_config_profile_validation": [
        "56-production-config-profile-drill.log",
    ],
    "production_release_decision_guard_validation": [
        "57-production-release-decision-guard.log",
    ],
    "live_canary_controlled_prep_validation": [
        "58-live-canary-controlled-prep-drill.log",
    ],
    "external_secret_provider_preflight_validation": [
        "59-external-secret-provider-preflight.log",
    ],
    "external_operator_approval_preflight_validation": [
        "60-external-operator-approval-preflight.log",
    ],
    "external_alert_routing_preflight_validation": [
        "61-external-alert-routing-preflight.log",
    ],
    "production_preflight_config_validation": [
        "62-production-preflight-config-guard.log",
    ],
    "production_preflight_config_fixture_validation": [
        "63-production-preflight-config-fixture-drill.log",
    ],
    "production_preflight_config_diff_review_validation": [
        "64-production-preflight-config-diff-review.log",
    ],
    "real_funds_canary_preflight_validation": [
        "65-real-funds-canary-preflight.log",
    ],
    "real_funds_canary_store_truth_cli_validation": [
        "72-real-funds-canary-store-truth-cli-preflight.log",
    ],
    "real_funds_canary_lifecycle_validation": [
        "66-real-funds-canary-lifecycle-drill.log",
    ],
    "real_funds_canary_ready_validation": [
        "67-real-funds-canary-ready-drill.log",
    ],
    "real_funds_canary_review_package_validation": [
        "68-real-funds-canary-review-package.log",
    ],
    "single_host_deployment_validation": [
        "69-single-host-deployment-drill.log",
    ],
    "single_host_canary_candidate_validation": [
        "70-single-host-canary-candidate-drill.log",
    ],
    "single_host_go_candidate_validation": [
        "71-single-host-go-candidate-drill.log",
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


def file_entry(path: Path, *, command: str | None = None) -> dict[str, str | int]:
    entry = log_entry(path)
    if command is not None:
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
    commands.update(
        {
            "72-real-funds-canary-store-truth-cli-preflight.log": (
                "python validation/run_real_funds_canary_store_truth_cli_preflight.py"
            )
        }
    )
    return commands


LOG_COMMANDS = load_log_commands()


def log_passed(path: Path) -> bool:
    text = path.read_text(errors="replace")
    if any(marker in text for marker in FAIL_MARKERS):
        return False
    if not json_log_semantics_ok(path, text):
        return False
    if not cargo_test_semantics_ok(path, text):
        return False
    if path.stat().st_size == 0:
        # cargo fmt and rustfmt success can produce an empty log.
        return path.name in {"01-cargo-fmt.log", "08-sdk-adapter-fmt.log"}
    return any(marker in text for marker in PASS_MARKERS) or path.name.endswith("-guard.log")


def json_log_semantics_ok(path: Path, text: str) -> bool:
    rule = JSON_LOG_RULES.get(path.name)
    if not rule:
        return True
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        return False
    return all(data.get(key) == expected for key, expected in rule.items())


def cargo_test_semantics_ok(path: Path, text: str) -> bool:
    rule = TEST_LOG_RULES.get(path.name)
    if not rule:
        return True
    if "running 0 tests" in text:
        return False
    summary = re.search(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
        r"(\d+) measured; (\d+) filtered out;",
        text,
    )
    if not summary:
        return False
    passed = int(summary.group(1))
    failed = int(summary.group(2))
    min_passed = int(rule["min_passed"])
    required_token = str(rule["required_token"])
    forbidden_token = str(rule.get("forbidden_token", ""))
    return (
        passed >= min_passed
        and failed == 0
        and required_token in text
        and (not forbidden_token or forbidden_token not in text)
    )


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
    artifact_sha256 = sha256(artifact_path) if artifact_path and artifact_path.exists() else None
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
            "name": None,
            "path": None,
            "sha256": None,
            "binding_note": "The canonical manifest is source-candidate evidence and does not self-bind a containing zip. Release artifacts are bound by external .zip.sha256 and .zip.evidence.json sidecars.",
        },
        "external_artifact_sidecar": {
            "name": artifact_path.name if artifact_path else None,
            "path": display_path(artifact_path) if artifact_path else None,
            "sha256": artifact_sha256,
            "sha256_sidecar": f"{artifact_path.name}.sha256" if artifact_path else None,
            "evidence_sidecar": f"{artifact_path.name}.evidence.json" if artifact_path else None,
            "binding_note": "Current workspace evidence binds the final release artifact here. When this manifest is archived inside the artifact, package_release.py normalizes this volatile hash to null to avoid archive self-reference.",
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
    if CONTRACT_VALIDATION_REPORT.exists():
        data["local_static_validation"]["contract_validation_report"] = file_entry(
            CONTRACT_VALIDATION_REPORT,
            command='python "${INTEGRATION_ROOT}/scripts/validate_contracts.py" --report-file "${EVIDENCE_DIR}/25-contract-validation.report.json"',
        )
    extra_logs = [path for path in sorted(log_dir.glob("*.log")) if path.name not in captured_names]
    data["additional_logs"] = [log_entry(path) for path in extra_logs]
    required_non_optional = [
        "local_static_validation",
        "rust_workspace_validation",
        "postgres_validation",
        "sdk_adapter_validation",
        "credentialed_non_trading_validation",
    ]
    skipped_sections = [
        section
        for section in [
            "postgres_validation",
            "credentialed_non_trading_validation",
        ]
        if data.get(section, {}).get("status") == "skipped"
    ]
    if skipped_sections:
        reason = (
            "Production-live-candidate local refresh. Current evidence binds source, "
            "Rust, PostgreSQL, SDK, local static, drill, governance, and artifact checks, but does not refresh "
            f"external sections skipped in this environment: {', '.join(skipped_sections)}. "
            "Production and live trading remain explicitly unapproved by default."
        )
    else:
        reason = (
            "Production-live-candidate. Required source, Rust, PostgreSQL, SDK, "
            "credentialed smoke, sign-only dry-run, local static, drill, governance, and artifact "
            "checks are bound in current evidence; production and live trading remain explicitly unapproved by default."
        )
    data["release_decision"] = {
        "validated_release": False,
        "status": "production-live-candidate",
        "production_ready": False,
        "live_trading_ready": False,
        "reason": reason,
        "required_non_optional_sections": required_non_optional,
    }
    OUT.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
