from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml


ENGINE_ROOT = Path(__file__).resolve().parents[1]
ROOT = ENGINE_ROOT.parent
VALIDATION_DIR = Path(__file__).resolve().parent
if str(VALIDATION_DIR) not in sys.path:
    sys.path.insert(0, str(VALIDATION_DIR))
INTEGRATION_SCRIPT_DIR = ROOT / "scripts"
if str(INTEGRATION_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(INTEGRATION_SCRIPT_DIR))

from validate_contracts_executor import (
    validate_store_and_backend_structure,
    validate_v04_source_landings,
    validate_v07_source_landings,
    validate_v08_dependency_and_sdk_policy,
    validate_v09_official_adapter_boundary,
    validate_v12_service_layer,
    validate_v15_admin_audit_and_runtime_provider,
    validate_v16_postgres_runtime_provider,
    validate_v19_redaction_and_live_guard,
    validate_v20_plan_storage_and_packaging,
    validate_v21_sign_only_and_runtime_models,
    validate_v23_lifecycle_query_and_hardening,
)
from validate_contracts_governance import (
    validate_canary_candidate_market_prep_boundary,
    validate_controlled_canary_release_decision_governance,
    validate_current_docs_and_release_governance,
    validate_current_evidence_manifest_guard,
    validate_current_hermes_client_surface,
    validate_single_host_deployment_governance,
    validate_v28_production_live_candidate_guard,
)
from validate_contracts_support import OPENAPI
from validate_contracts_surface import (
    validate_additional_properties,
    validate_critical_contract_shapes,
    validate_no_public_forbidden_tokens,
    validate_paths_and_statuses,
    validate_python_field_parity,
    validate_rust_deny_unknown_fields,
    validate_sql_idempotency,
)


@dataclass(frozen=True)
class ValidatorSpec:
    id: str
    category: str
    severity: str
    proof_mode: str
    uses_spec: bool
    fn: object


VALIDATORS = [
    ValidatorSpec("paths_and_statuses", "surface", "S2", "structured", True, validate_paths_and_statuses),
    ValidatorSpec("critical_contract_shapes", "surface", "S1", "structured", True, validate_critical_contract_shapes),
    ValidatorSpec("no_public_forbidden_tokens", "surface", "S1", "mixed", True, validate_no_public_forbidden_tokens),
    ValidatorSpec("additional_properties", "surface", "S2", "structured", True, validate_additional_properties),
    ValidatorSpec("python_field_parity", "surface", "S2", "structured", True, validate_python_field_parity),
    ValidatorSpec("sql_idempotency", "surface", "S1", "structured", False, validate_sql_idempotency),
    ValidatorSpec("rust_deny_unknown_fields", "surface", "S2", "structured", False, validate_rust_deny_unknown_fields),
    ValidatorSpec("v04_source_landings", "executor", "S2", "mixed", False, validate_v04_source_landings),
    ValidatorSpec("v07_source_landings", "executor", "S2", "structured", False, validate_v07_source_landings),
    ValidatorSpec("v08_dependency_and_sdk_policy", "executor", "S2", "structured", False, validate_v08_dependency_and_sdk_policy),
    ValidatorSpec("v09_official_adapter_boundary", "executor", "S1", "mixed", False, validate_v09_official_adapter_boundary),
    ValidatorSpec("v12_service_layer", "executor", "S2", "structured", True, validate_v12_service_layer),
    ValidatorSpec("v15_admin_audit_and_runtime_provider", "executor", "S2", "mixed", True, validate_v15_admin_audit_and_runtime_provider),
    ValidatorSpec("v16_postgres_runtime_provider", "executor", "S1", "mixed", True, validate_v16_postgres_runtime_provider),
    ValidatorSpec("v19_redaction_and_live_guard", "executor", "S1", "structured", True, validate_v19_redaction_and_live_guard),
    ValidatorSpec("v20_plan_storage_and_packaging", "executor", "S2", "mixed", True, validate_v20_plan_storage_and_packaging),
    ValidatorSpec("v21_sign_only_and_runtime_models", "executor", "S2", "structured", True, validate_v21_sign_only_and_runtime_models),
    ValidatorSpec("store_and_backend_structure", "executor", "S2", "structured", False, validate_store_and_backend_structure),
    ValidatorSpec("v23_lifecycle_query_and_hardening", "executor", "S1", "mixed", True, validate_v23_lifecycle_query_and_hardening),
    ValidatorSpec("current_hermes_client_surface", "governance", "S2", "structured", False, validate_current_hermes_client_surface),
    ValidatorSpec("current_evidence_manifest_guard", "governance", "S1", "structured", False, validate_current_evidence_manifest_guard),
    ValidatorSpec("current_docs_and_release_governance", "governance", "S1", "structured", False, validate_current_docs_and_release_governance),
    ValidatorSpec("controlled_canary_release_decision_governance", "governance", "S1", "mixed", False, validate_controlled_canary_release_decision_governance),
    ValidatorSpec("canary_candidate_market_prep_boundary", "governance", "S2", "mixed", False, validate_canary_candidate_market_prep_boundary),
    ValidatorSpec("single_host_deployment_governance", "governance", "S1", "mixed", False, validate_single_host_deployment_governance),
    ValidatorSpec("v28_production_live_candidate_guard", "governance", "S1", "mixed", False, validate_v28_production_live_candidate_guard),
]


def load_openapi_spec() -> dict[str, Any]:
    return yaml.safe_load(OPENAPI.read_text())


def error_message(exc: BaseException) -> str:
    text = str(exc).strip()
    if text:
        return text
    return exc.__class__.__name__


def run_validator(validator: ValidatorSpec, spec: dict[str, Any]) -> dict[str, str]:
    try:
        if validator.uses_spec:
            validator.fn(spec)
        else:
            validator.fn()
    except BaseException as exc:
        return {
            "id": validator.id,
            "category": validator.category,
            "severity": validator.severity,
            "proof_mode": validator.proof_mode,
            "status": "fail",
            "error_type": exc.__class__.__name__,
            "error": error_message(exc),
        }
    return {
        "id": validator.id,
        "category": validator.category,
        "severity": validator.severity,
        "proof_mode": validator.proof_mode,
        "status": "pass",
    }


def build_report(spec: dict[str, Any], validators: list[ValidatorSpec] | None = None) -> dict[str, Any]:
    active_validators = validators or VALIDATORS
    checks = []
    for validator in active_validators:
        checks.append(run_validator(validator, spec))
    failed_checks = [check for check in checks if check["status"] != "pass"]
    proof_mode_counts: dict[str, int] = {}
    severity_counts: dict[str, int] = {}
    failed_severity_counts: dict[str, int] = {}
    for check in checks:
        proof_mode = str(check["proof_mode"])
        proof_mode_counts[proof_mode] = proof_mode_counts.get(proof_mode, 0) + 1
        severity = str(check["severity"])
        severity_counts[severity] = severity_counts.get(severity, 0) + 1
        if check["status"] != "pass":
            failed_severity_counts[severity] = failed_severity_counts.get(severity, 0) + 1
    return {
        "status": "ok" if not failed_checks else "fail",
        "paths": len(spec["paths"]),
        "schemas": len(spec["components"]["schemas"]),
        "check_count": len(checks),
        "failed_check_count": len(failed_checks),
        "failed_check_ids": [check["id"] for check in failed_checks],
        "proof_mode_counts": proof_mode_counts,
        "severity_counts": severity_counts,
        "failed_severity_counts": failed_severity_counts,
        "checks": checks,
    }


def write_report(report: dict[str, Any], report_file: str | Path) -> None:
    path = Path(report_file)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2, sort_keys=True)
        fh.write("\n")
