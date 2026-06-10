from __future__ import annotations

import inspect
import json
import re
import sys
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace


ENGINE_ROOT = Path(__file__).resolve().parents[1]
ROOT = ENGINE_ROOT.parent
VALIDATION_DIR = Path(__file__).resolve().parent
if str(VALIDATION_DIR) not in sys.path:
    sys.path.insert(0, str(VALIDATION_DIR))
INTEGRATION_SCRIPT_DIR = ROOT / "scripts"
if str(INTEGRATION_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(INTEGRATION_SCRIPT_DIR))

from validate_contracts_support import (
    CONTROL,
    CORE_SRC,
    EXECUTOR,
    EXCLUDED_PREFIXES,
    OPENAPI,
    ROOT,
    SDK_ADAPTER_SRC,
    fail,
    import_control_client,
    import_control_models,
    import_module_from_path,
    python_function_body,
    rust_source_text,
)


def require_existing_paths(paths: list, label: str) -> None:
    for path in paths:
        if not path.exists():
            fail(f"{label} missing: {path.relative_to(ROOT)}")


def validate_absent_tokens(text: str, label: str, tokens: list[str]) -> None:
    for token in tokens:
        if token in text:
            fail(f"{label} contains forbidden token: {token}")


def validate_current_hermes_client_surface() -> None:
    client_module = import_control_client()
    models = import_control_models()
    client_cls = client_module.ExecutorClient

    def require_method(name: str, *, required_params: list[str], return_type_name: str) -> None:
        method = getattr(client_cls, name, None)
        if method is None:
            fail(f"Hermes current client surface missing method: {name}")
        signature = inspect.signature(method)
        params = signature.parameters
        for param in required_params:
            if param not in params:
                fail(f"Hermes current client surface method {name} missing parameter: {param}")
        if return_type_name not in str(signature.return_annotation):
            fail(f"Hermes current client surface method {name} must return {return_type_name}")

    require_method(
        "record_sign_only_lifecycle_event",
        required_params=["record", "correlation_id"],
        return_type_name="SignOnlyLifecycleRecord",
    )
    require_method(
        "list_sign_only_lifecycle_events",
        required_params=["execution_id", "before_event_id", "correlation_id"],
        return_type_name="SignOnlyLifecycleRecord",
    )
    require_method(
        "list_execution_lifecycle_events",
        required_params=["execution_id", "before_event_id", "correlation_id"],
        return_type_name="ExecutionLifecycleEvent",
    )
    require_method(
        "list_admin_audit_events",
        required_params=["principal_subject", "result", "audit_correlation_id", "correlation_id"],
        return_type_name="AdminAuditEvent",
    )
    require_method(
        "reconcile_order_local",
        required_params=["account_id", "order_id", "remote_observation", "reason", "correlation_id"],
        return_type_name="ReconcileOrderLocalResponse",
    )
    require_method(
        "cancel_order",
        required_params=["account_id", "order_id", "reason", "execution_id", "correlation_id"],
        return_type_name="CancelReceipt",
    )
    require_method(
        "reconcile",
        required_params=["account_id", "reason", "execution_id", "correlation_id"],
        return_type_name="ReconcileReport",
    )

    headers_owner = client_cls.__new__(client_cls)
    headers_owner.config = SimpleNamespace(service_token="svc", admin_token="adm")
    headers = client_cls._headers(headers_owner, correlation_id="corr-123")
    if headers.get("X-Correlation-Id") != "corr-123":
        fail("Hermes current client surface must propagate X-Correlation-Id header")

    required_models: dict[str, set[str]] = {
        "SignOnlyLifecycleRecord": {"execution_id", "client_event_id", "signed_order_ref", "no_remote_side_effect"},
        "RedactedPayloadEnvelope": {"correlation_id", "redacted_fields", "body"},
        "ExecutionLifecycleEvent": {"execution_id", "payload"},
        "AdminAuditEvent": {"principal_subject", "result", "correlation_id"},
        "OrderLifecycleDivergence": set(),
        "ReconcileOrderLocalResponse": set(),
    }
    for model_name, required_fields in required_models.items():
        model = getattr(models, model_name, None)
        if model is None:
            fail(f"Hermes current model surface missing model: {model_name}")
        model_fields = getattr(model, "model_fields", {})
        for field in required_fields:
            if field not in model_fields:
                fail(f"Hermes current model surface {model_name} missing field: {field}")

    payload_annotation = str(models.ExecutionLifecycleEvent.model_fields["payload"].annotation)
    if "RedactedPayloadEnvelope" not in payload_annotation:
        fail("Hermes current model surface ExecutionLifecycleEvent.payload must bind RedactedPayloadEnvelope")

    try:
        models.SignOnlyLifecycleRecord.model_validate(
            {
                "execution_id": "exec-1",
                "account_id": "acct-1",
                "state": "ABANDONED",
                "event": "ABANDON",
                "signed_order_ref": None,
                "no_remote_side_effect": False,
            }
        )
    except Exception as exc:
        if "sign-only lifecycle records must not contain remote side effects" not in str(exc):
            fail("Hermes current model surface SignOnlyLifecycleRecord must reject remote side effects with the expected boundary")
    else:
        fail("Hermes current model surface SignOnlyLifecycleRecord must reject remote side effects")


def validate_current_evidence_manifest_guard() -> None:
    manifest = EXECUTOR / "validation/templates/evidence_manifest.template.json"
    current_manifest = EXECUTOR / "evidence/current/manifest.json"
    guard = EXECUTOR / "validation/check_current_evidence_manifest.py"
    governance_guard = EXECUTOR / "validation/check_docs_evidence_governance.py"
    writer = EXECUTOR / "validation/write_current_evidence_manifest.py"
    if not manifest.exists():
        fail("current evidence manifest template missing from validation/templates")
    if not guard.exists():
        fail("current evidence manifest guard missing")
    if not governance_guard.exists():
        fail("current docs/evidence governance guard missing")
    if not writer.exists():
        fail("current evidence manifest writer missing")
    data = json.loads(manifest.read_text())
    expected_version = (ROOT / "VERSION").read_text().strip()
    if data.get("version") != expected_version:
        fail(f"current evidence manifest template must use version {expected_version}")
    if data.get("canonical_evidence_dir") != "polymarket-execution-engine/evidence/current":
        fail("current evidence manifest template must point to evidence/current")
    if not current_manifest.exists():
        fail("current evidence manifest missing")
    if data.get("release_decision", {}).get("validated_release") is not False:
        fail("current evidence template must not mark validated_release=true")
    for section in [
        "local_static_validation",
        "rust_workspace_validation",
        "postgres_validation",
        "sdk_adapter_validation",
        "credentialed_non_trading_validation",
    ]:
        if data.get(section, {}).get("status") != "pending":
            fail(f"current evidence template {section} must stay pending")
    guard_module = import_module_from_path("pmx_check_current_evidence_manifest", guard)
    writer_module = import_module_from_path("pmx_write_current_evidence_manifest", writer)
    docs_module = import_module_from_path("pmx_check_docs_evidence_governance", governance_guard)

    if set(getattr(guard_module, "REQUIRED_SECTIONS", [])) != {
        "local_static_validation",
        "rust_workspace_validation",
        "postgres_validation",
        "sdk_adapter_validation",
        "credentialed_non_trading_validation",
    }:
        fail("current evidence guard REQUIRED_SECTIONS drifted from canonical release gate set")
    if set(getattr(guard_module, "VALID_STATUSES", set())) != {"pending", "pass", "fail", "skipped", "not_run"}:
        fail("current evidence guard VALID_STATUSES drifted")
    if getattr(guard_module, "TEST_LOG_RULES", None) != getattr(writer_module, "TEST_LOG_RULES", None):
        fail("current evidence guard TEST_LOG_RULES must match manifest writer")
    if getattr(guard_module, "JSON_LOG_RULES", None) != getattr(writer_module, "JSON_LOG_RULES", None):
        fail("current evidence guard JSON_LOG_RULES must match manifest writer")

    writer_sections = getattr(writer_module, "SECTIONS", {})
    if not isinstance(writer_sections, dict):
        fail("current evidence manifest writer must export SECTIONS")
    for section in [
        "local_static_validation",
        "runtime_worker_status_validation",
        "real_funds_canary_store_truth_cli_validation",
    ]:
        if section not in writer_sections:
            fail(f"current evidence manifest writer missing section: {section}")
    if getattr(writer_module, "CURRENT_DIR", None) != EXECUTOR / "evidence" / "current":
        fail("current evidence manifest writer CURRENT_DIR must bind evidence/current")
    if getattr(writer_module, "OUT", None) != EXECUTOR / "evidence" / "current" / "manifest.json":
        fail("current evidence manifest writer OUT must bind evidence/current/manifest.json")
    if not callable(getattr(writer_module, "build_section", None)):
        fail("current evidence manifest writer missing build_section")
    if not callable(getattr(guard_module, "validate", None)):
        fail("current evidence guard missing validate()")
    if not callable(getattr(guard_module, "validate_test_log_semantics", None)):
        fail("current evidence guard missing validate_test_log_semantics()")
    if not callable(getattr(guard_module, "validate_json_log_semantics", None)):
        fail("current evidence guard missing validate_json_log_semantics()")

    if getattr(docs_module, "CURRENT_MANIFEST", None) != EXECUTOR / "evidence" / "current" / "manifest.json":
        fail("current docs/evidence governance guard must bind canonical current manifest")
    if getattr(docs_module, "RELEASE_MANIFEST", None) != EXECUTOR / "release" / "manifest.json":
        fail("current docs/evidence governance guard must bind release manifest")
    if getattr(docs_module, "PACKAGE_SCRIPT", None) != ROOT / "scripts" / "package_release.py":
        fail("current docs/evidence governance guard must bind package_release.py")
    if getattr(docs_module, "ARTIFACT_CHECK", None) != ROOT / "scripts" / "check_release_artifact.py":
        fail("current docs/evidence governance guard must bind check_release_artifact.py")
    if getattr(docs_module, "RELEASE_POLICY", None) != ROOT / "scripts" / "release_policy.py":
        fail("current docs/evidence governance guard must bind release_policy.py")
    for func_name in [
        "validate_root_docs",
        "validate_evidence_layout",
        "validate_release_binding",
        "validate_current_manifest",
        "validate_execution_docs_and_gates",
        "validate_agents_guidance",
    ]:
        if not callable(getattr(docs_module, func_name, None)):
            fail(f"current docs/evidence governance guard missing function: {func_name}")


def validate_current_docs_and_release_governance() -> None:
    release = json.loads((EXECUTOR / "release/manifest.json").read_text())
    expected_archive_prefixes = {
        "docs/archive",
        "external_reviews",
        "validation/archive",
        "polymarket-execution-engine/validation/archive",
        "polymarket-execution-engine/evidence/archive",
        "polymarket-execution-engine/docs/archive",
    }
    if not expected_archive_prefixes.issubset(EXCLUDED_PREFIXES):
        fail("release policy missing expected archive exclusion prefixes")
    canonical = release.get("canonical_evidence", {})
    if canonical.get("manifest_path") != "polymarket-execution-engine/evidence/current/manifest.json":
        fail("release manifest must bind canonical evidence manifest")
    if canonical.get("historical_evidence_policy") != "archive-excluded-from-release-package":
        fail("release manifest must state archive-excluded-from-release-package")
    stale_root = []
    historical_root = []
    for path in ROOT.glob("*.md"):
        if path.name.startswith("V0_") or path.name.startswith("VALIDATION_V0_"):
            stale_root.append(path.name)
        first_line = path.read_text(errors="replace").splitlines()[:1]
        if first_line and re.search(r"\bHistorical v0\.", first_line[0], re.IGNORECASE):
            historical_root.append(path.name)
    if stale_root:
        fail("stale versioned root docs remain outside docs/archive: " + ", ".join(sorted(stale_root)))
    if historical_root:
        fail("historical version root docs remain outside docs/archive: " + ", ".join(sorted(historical_root)))
    active_versioned_engine_docs = [path.name for path in (EXECUTOR / "docs").glob("V0_*.md")]
    if active_versioned_engine_docs:
        fail(
            "stale execution-engine versioned docs remain outside docs/archive: "
            + ", ".join(sorted(active_versioned_engine_docs))
        )

    docs_guard = EXECUTOR / "validation/check_docs_evidence_governance.py"
    docs_module = import_module_from_path("pmx_check_docs_evidence_governance_docs", docs_guard)
    if getattr(docs_module, "PACKAGE_SCRIPT", None) != ROOT / "scripts" / "package_release.py":
        fail("docs/evidence governance guard must bind package_release.py")
    if getattr(docs_module, "ARTIFACT_CHECK", None) != ROOT / "scripts" / "check_release_artifact.py":
        fail("docs/evidence governance guard must bind check_release_artifact.py")
    if getattr(docs_module, "RELEASE_POLICY", None) != ROOT / "scripts" / "release_policy.py":
        fail("docs/evidence governance guard must bind release_policy.py")
    for fn_name in [
        "validate_root_docs",
        "validate_evidence_layout",
        "validate_release_binding",
        "validate_current_manifest",
        "validate_execution_docs_and_gates",
        "validate_agents_guidance",
        "validate_packaging_scripts",
    ]:
        if not callable(getattr(docs_module, fn_name, None)):
            fail(f"docs/evidence governance guard missing callable: {fn_name}")
    active_old_gates = [path.name for path in (EXECUTOR / "validation").glob("run_v0_*_gates.sh")]
    if active_old_gates:
        fail("stale gate scripts remain outside validation/archive: " + ", ".join(sorted(active_old_gates)))
    if not (EXECUTOR / "validation/run_current_gates.sh").exists():
        fail("run_current_gates.sh missing")
    if (EXECUTOR / "evidence/v0.23").exists():
        fail("evidence/v0.23 must not exist; template belongs in validation/templates")
    todo_artifacts = [path.relative_to(ROOT).as_posix() for path in (EXECUTOR / "validation").rglob("*todo*")]
    if todo_artifacts:
        fail("validation TODO artifacts remain: " + ", ".join(sorted(todo_artifacts)))


def validate_controlled_canary_release_decision_governance() -> None:
    template = EXECUTOR / "config/controlled-canary.release-decision.template.json"
    example = EXECUTOR / "config/controlled-canary.release-decision.example.json"
    invalid = EXECUTOR / "config/controlled-canary.release-decision.invalid-partial.fixture.json"
    invalid_mismatched = EXECUTOR / "config/controlled-canary.release-decision.invalid-mismatched.fixture.json"
    validator = EXECUTOR / "validation/validate_controlled_canary_release_decision.py"
    review_script = EXECUTOR / "validation/prepare_real_funds_canary_review.py"
    review_drill = EXECUTOR / "validation/run_real_funds_canary_review_package_drill.py"
    readiness_doc = EXECUTOR / "docs/REAL_FUNDS_CANARY_OPERATIONS_READINESS.md"
    rehearsal = EXECUTOR / "validation/run_real_funds_canary_blocked_rehearsal_package.py"
    external_template = EXECUTOR / "config/controlled-canary.external-references.template.json"
    external_example = EXECUTOR / "config/controlled-canary.external-references.example.json"
    external_invalid = EXECUTOR / "config/controlled-canary.external-references.invalid-sensitive.fixture.json"
    external_validator = EXECUTOR / "validation/validate_controlled_canary_external_references.py"
    runtime_truth_template = EXECUTOR / "config/controlled-canary.runtime-truth.template.json"
    runtime_truth_invalid_partial = EXECUTOR / "config/controlled-canary.runtime-truth.invalid-partial.fixture.json"
    runtime_truth_invalid_sensitive = EXECUTOR / "config/controlled-canary.runtime-truth.invalid-sensitive.fixture.json"
    runtime_truth_validator = EXECUTOR / "validation/validate_controlled_canary_runtime_truth.py"
    require_existing_paths(
        [template, example, invalid, invalid_mismatched, validator],
        "controlled canary release-decision governance file",
    )
    require_existing_paths(
        [external_template, external_example, external_invalid, external_validator],
        "controlled canary external-reference governance file",
    )
    require_existing_paths(
        [
            runtime_truth_template,
            runtime_truth_invalid_partial,
            runtime_truth_invalid_sensitive,
            runtime_truth_validator,
        ],
        "controlled canary runtime-truth governance file",
    )
    template_data = json.loads(template.read_text())
    example_data = json.loads(example.read_text())
    invalid_data = json.loads(invalid.read_text())
    invalid_mismatched_data = json.loads(invalid_mismatched.read_text())
    external_example_data = json.loads(external_example.read_text())
    runtime_truth_template_data = json.loads(runtime_truth_template.read_text())
    if template_data.get("decision") != "no_go":
        fail("controlled canary release-decision template must default to no_go")
    for flag in [
        "live_submit_authorized",
        "live_cancel_authorized",
        "production_deployment_authorized",
        "real_funds_canary_authorized",
        "remote_side_effects_authorized",
        "allow_real_funds_canary",
    ]:
        if template_data.get(flag) is not False:
            fail(f"controlled canary release-decision template must keep {flag}=false")
        if example_data.get(flag) is not False:
            fail(f"controlled canary release-decision example must keep {flag}=false")
    if example_data.get("artifact_sha256") != "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb":
        fail("controlled canary release-decision example must bind illustrative current-release artifact SHA-256")
    if example_data.get("market_candidate_sha256") != "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd":
        fail("controlled canary release-decision example must bind illustrative current-release market candidate SHA-256")
    if invalid_data.get("decision") != "go" or invalid_data.get("live_submit_authorized") is not True:
        fail("controlled canary invalid partial fixture must exercise rejected go/live-submit path")
    if invalid_mismatched_data.get("artifact_sha256") == example_data.get("artifact_sha256"):
        fail("controlled canary invalid mismatched fixture must use a mismatched artifact hash")
    validator_module = import_module_from_path(
        "pmx_validate_controlled_canary_release_decision", validator
    )
    if getattr(validator_module, "TEMPLATE", None) != template:
        fail("controlled canary release-decision validator must bind canonical template")
    if getattr(validator_module, "EXAMPLE", None) != example:
        fail("controlled canary release-decision validator must bind canonical example")
    if getattr(validator_module, "INVALID_PARTIAL", None) != invalid:
        fail("controlled canary release-decision validator must bind invalid partial fixture")
    if getattr(validator_module, "INVALID_MISMATCHED", None) != invalid_mismatched:
        fail("controlled canary release-decision validator must bind invalid mismatched fixture")
    if getattr(validator_module, "EXPECTED_RUN_IDS", None) != {
        "root_ci_run_id": "26268697168",
        "hermes_ci_run_id": "26267887116",
        "execution_engine_ci_run_id": "26268276210",
        "credentialed_sdk_run_id": "local-current-gates-20260523",
    }:
        fail("controlled canary release-decision validator EXPECTED_RUN_IDS drifted")
    if "reviewed_release_decision_present" not in getattr(
        validator_module, "ALLOWED_TOP_LEVEL_FIELDS", set()
    ):
        fail("controlled canary release-decision validator must require reviewed_release_decision_present")
    if "real_funds_canary_authorized" not in getattr(
        validator_module, "AUTHORIZATION_FLAGS", []
    ):
        fail("controlled canary release-decision validator AUTHORIZATION_FLAGS drifted")
    if not callable(getattr(validator_module, "validate_shape", None)):
        fail("controlled canary release-decision validator missing validate_shape()")
    if not callable(getattr(validator_module, "main", None)):
        fail("controlled canary release-decision validator missing main()")

    review_module = import_module_from_path(
        "pmx_prepare_real_funds_canary_review", review_script
    )
    if getattr(review_module, "DEFAULT_RELEASE_DECISION", None) != template:
        fail("real-funds canary review package must bind release-decision template")
    if getattr(review_module, "DEFAULT_EXTERNAL_REFERENCES", None) != external_template:
        fail("real-funds canary review package must bind external-references template")
    if getattr(review_module, "DEFAULT_ROOT_CI_RUN_ID", None) != "26268697168":
        fail("real-funds canary review package must bind default root CI run id")
    if getattr(review_module, "DEFAULT_HERMES_CI_RUN_ID", None) != "26267887116":
        fail("real-funds canary review package must bind default Hermes CI run id")
    if getattr(review_module, "DEFAULT_EXECUTION_ENGINE_CI_RUN_ID", None) != "26268276210":
        fail("real-funds canary review package must bind default execution-engine CI run id")
    if getattr(review_module, "DEFAULT_CREDENTIALED_SDK_RUN_ID", None) != "local-current-gates-20260523":
        fail("real-funds canary review package must bind default credentialed SDK run id")
    for fn_name in [
        "resolve_input_path",
        "require_sha256",
        "validate_candidate_market_json",
        "main",
    ]:
        if not callable(getattr(review_module, fn_name, None)):
            fail(f"real-funds canary review package missing callable: {fn_name}")
    review_text = review_script.read_text()
    review_main_body = python_function_body(review_text, "main")
    for needle in [
        "--external-references-file",
        "--artifact-sha256",
        "--evidence-manifest-sha256",
        "external_references_placeholders_remaining",
        "validate_external_references_shape(",
        "release sidecar binds the final zip hash",
    ]:
        if needle not in review_main_body:
            fail(f"real-funds canary review package main missing token: {needle}")
    drill_module = import_module_from_path(
        "pmx_run_real_funds_canary_review_package_drill", review_drill
    )
    if getattr(drill_module, "SCRIPT", None) != review_script:
        fail("real-funds canary review package drill must bind package script")
    if getattr(drill_module, "DECISION_VALIDATOR", None) != validator:
        fail("real-funds canary review package drill must bind release-decision validator")
    if getattr(drill_module, "EXTERNAL_REFERENCES_VALIDATOR", None) != external_validator:
        fail("real-funds canary review package drill must bind external-reference validator")
    if getattr(drill_module, "BLOCKED_REHEARSAL", None) != rehearsal:
        fail("real-funds canary review package drill must bind blocked rehearsal script")
    if getattr(drill_module, "EXTERNAL_REFERENCES_EXAMPLE", None) != external_example:
        fail("real-funds canary review package drill must bind external-reference example")
    if getattr(drill_module, "EXTERNAL_REFERENCES_TEMPLATE", None) != external_template:
        fail("real-funds canary review package drill must bind external-reference template")
    if getattr(drill_module, "DOC", None) != readiness_doc:
        fail("real-funds canary review package drill must bind readiness doc")
    if not callable(getattr(drill_module, "main", None)):
        fail("real-funds canary review package drill missing main()")
    drill_text = review_drill.read_text()
    if getattr(drill_module, "EXAMPLE_REVIEW_ARTIFACT_SHA256", None) != "c0c22c91541d48c508a588b06a2fa5d7051bc6c8e29df626de67a59cc96c24e6":
        fail("real-funds canary review package drill must bind example review artifact sha256")
    if getattr(drill_module, "MANIFEST_WRITER", None) != EXECUTOR / "validation" / "write_current_evidence_manifest.py":
        fail("real-funds canary review package drill must bind manifest writer")
    drill_main_body = python_function_body(drill_text, "main")
    for needle in [
        "--file",
        "--allow-placeholders",
        "must reject unresolved placeholders",
        "review-with-concrete-references",
        "68-real-funds-canary-review-package.log",
        '"review_package_only_not_armed_approval"',
    ]:
        if needle not in drill_main_body:
            fail(f"real-funds canary review package drill main missing token: {needle}")
    external_module = import_module_from_path(
        "pmx_validate_controlled_canary_external_references", external_validator
    )
    if getattr(external_module, "EXPECTED_ARTIFACT_SHA256", None) != (
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ):
        fail("controlled canary external-reference validator EXPECTED_ARTIFACT_SHA256 drifted")
    if "credentialed_sdk_run_id" not in getattr(
        external_module, "EXPECTED_RUN_IDS", {}
    ):
        fail("controlled canary external-reference validator EXPECTED_RUN_IDS drifted")
    for fn_name in ["validate_shape", "placeholder_paths", "has_placeholder", "main"]:
        if not callable(getattr(external_module, fn_name, None)):
            fail(f"controlled canary external-reference validator missing callable: {fn_name}")
    external_text = external_validator.read_text()
    for const_name, expected in [
        ("TEMPLATE", external_template),
        ("EXAMPLE", external_example),
        ("INVALID_SENSITIVE", external_invalid),
    ]:
        if getattr(external_module, const_name, None) != expected:
            fail(f"controlled canary external-reference validator must bind {const_name}")
    if "rollback_runbook_ref" not in getattr(external_module, "REQUIRED_FIELDS", {}).get("runbooks", []):
        fail("controlled canary external-reference validator REQUIRED_FIELDS must include rollback_runbook_ref")
    if "fixture-sensitive-value-must-not-be-logged" not in getattr(external_module, "FORBIDDEN_VALUE_FRAGMENTS", ()):
        fail("controlled canary external-reference validator FORBIDDEN_VALUE_FRAGMENTS drifted")
    if "SignedOrderEnvelope" not in getattr(external_module, "FORBIDDEN_KEYS", set()):
        fail("controlled canary external-reference validator FORBIDDEN_KEYS drifted")
    external_main_body = python_function_body(external_text, "main")
    for needle in [
        "--allow-placeholders",
        "Validate an operator-supplied external reference candidate",
        "invalid sensitive fixture must be rejected",
        "references_only_no_secret_values",
        "placeholder_paths(candidate)",
    ]:
        if needle not in external_main_body:
            fail(f"controlled canary external-reference validator main missing token: {needle}")
    validate_no_sensitive_body = python_function_body(
        external_text, "validate_no_sensitive_material"
    )
    for needle in [
        "forbidden sensitive reference key",
        "forbidden sensitive-looking reference value",
        "FORBIDDEN_KEYS",
        "FORBIDDEN_VALUE_FRAGMENTS",
    ]:
        if needle not in validate_no_sensitive_body:
            fail(f"controlled canary external-reference validator validate_no_sensitive_material missing token: {needle}")
    if external_example_data.get("artifact_sha256") != example_data.get("artifact_sha256"):
        fail("controlled canary external-reference example must bind the same artifact hash as the release-decision example")
    if external_example_data.get("evidence_manifest_sha256") != example_data.get("evidence_manifest_sha256"):
        fail("controlled canary external-reference example must bind the same evidence manifest hash as the release-decision example")
    if runtime_truth_template_data.get("schema_version") != 1:
        fail("controlled canary runtime-truth template must use schema_version=1")
    if runtime_truth_template_data.get("references_only_no_secret_values") is not True:
        fail("controlled canary runtime-truth template must be references-only")
    for flag in [
        "live_submit_allowed",
        "live_cancel_allowed",
        "real_funds_canary_authorized",
        "remote_side_effects",
        "production_ready_claimed",
    ]:
        if runtime_truth_template_data.get(flag) is not False:
            fail(f"controlled canary runtime-truth template must keep {flag}=false")
    runtime_truth_dependencies = runtime_truth_template_data.get("dependencies")
    if not isinstance(runtime_truth_dependencies, list):
        fail("controlled canary runtime-truth template dependencies must be a list")
        runtime_truth_dependencies = []
    dependency_names = {item.get("name") for item in runtime_truth_dependencies if isinstance(item, dict)}
    for name in ["kill_switch", "live_submit_gate", "idempotency_lease", "order_cancel_reconciliation"]:
        if name not in dependency_names:
            fail(f"controlled canary runtime-truth template missing dependency: {name}")
    for item in runtime_truth_dependencies:
        if not isinstance(item, dict):
            fail("controlled canary runtime-truth template dependencies must be objects")
            continue
        if item.get("status") != "durable_runtime_truth":
            fail(f"controlled canary runtime-truth template dependency {item.get('name')} must require durable_runtime_truth")
        evidence_ref = item.get("evidence_ref")
        if not isinstance(evidence_ref, str) or "REPLACE_WITH" not in evidence_ref:
            fail(f"controlled canary runtime-truth template dependency {item.get('name')} must use placeholder evidence_ref")
    runtime_truth_module = import_module_from_path(
        "pmx_validate_controlled_canary_runtime_truth", runtime_truth_validator
    )
    for fn_name in ["validate_shape", "placeholder_paths", "has_placeholder", "main"]:
        if not callable(getattr(runtime_truth_module, fn_name, None)):
            fail(f"controlled canary runtime-truth validator missing callable: {fn_name}")
    runtime_truth_validator_text = runtime_truth_validator.read_text()
    for needle in [
        "Validate an operator-supplied runtime truth candidate",
        "--allow-placeholders",
        "runtime truth missing durable dependencies",
        "invalid partial fixture must be rejected",
        "invalid sensitive fixture must be rejected",
        "forbidden sensitive runtime-truth key",
        "references_only_no_secret_values",
    ]:
        if needle not in runtime_truth_validator_text:
            fail(f"controlled canary runtime-truth validator missing token: {needle}")
    gate_text = (EXECUTOR / "validation/run_current_gates_impl.sh").read_text()
    if "73-controlled-canary-runtime-truth.log" not in gate_text:
        fail("current gates must emit controlled canary runtime-truth validator log")
    store_truth_cli_preflight_text = (EXECUTOR / "validation/run_real_funds_canary_store_truth_cli_preflight.py").read_text()
    for needle in [
        "--runtime-truth-output",
        "--artifact-sha256",
        "--workspace-manifest-sha256",
        "--archived-manifest-sha256",
        "runtime_truth_document",
        "references_only_no_secret_values",
        "pg://canary-runtime-truth",
    ]:
        if needle not in store_truth_cli_preflight_text:
            fail(f"store truth CLI preflight missing runtime-truth output token: {needle}")
    controlled_pipeline_text = (ROOT / "scripts/run_controlled_canary_pipeline.py").read_text()
    for needle in [
        "validate_controlled_canary_runtime_truth.py",
        "runtime truth validator failed",
        "runtime truth artifact binding mismatch",
        "expected_artifact_sha256",
        "expected_workspace_manifest_sha256",
        "expected_archived_manifest_sha256",
    ]:
        if needle not in controlled_pipeline_text:
            fail(f"controlled canary pipeline missing runtime-truth binding token: {needle}")
    readiness_text = readiness_doc.read_text()
    if not rehearsal.exists():
        fail("real-funds canary blocked rehearsal package script missing")
    rehearsal_text = rehearsal.read_text()
    for needle in [
        "blocked_real_funds_canary_armed_no_go",
        "run_reviewed_go_canary_armed.py",
        "reviewed-go decision invalid",
        "decision must be go",
        "release_decision_gate",
        "remote_side_effects",
        "raw_signed_order_exposed",
        "--output-dir",
        "blocked-rehearsal.report.json",
        "--package-dir",
        "--env-file",
    ]:
        if needle not in rehearsal_text:
            fail(f"blocked real-funds canary rehearsal script missing token: {needle}")
    for needle in [
        "default no-go",
        "external-references.json",
        "release-decision.json",
        "controlled-canary.runtime-truth.template.json",
        "real_funds_canary_authorized=false",
        "--external-references-file",
        "REPLACE_WITH_*",
        "run_real_funds_canary_blocked_rehearsal_package.py",
    ]:
        if needle not in readiness_text:
            fail(f"real-funds canary operations readiness doc missing token: {needle}")
    for needle in ["prepare_canary_candidate_market.py", "candidate-market.audit.json", "read-only public API candidate"]:
        if needle not in readiness_text:
            fail(f"real-funds canary operations readiness doc missing candidate-prep token: {needle}")
    for needle in ["prepare_canary_candidate_market.py", "candidate-market.audit.json"]:
        if needle not in review_text:
            fail(f"real-funds canary review package missing candidate-prep token: {needle}")


def validate_canary_candidate_market_prep_boundary() -> None:
    prep_script = EXECUTOR / "validation/prepare_canary_candidate_market.py"
    if not prep_script.exists():
        fail("execution-engine canary candidate market prep script missing")
    prep_module = import_module_from_path("pmx_prepare_canary_candidate_market", prep_script)
    if getattr(prep_module, "ROOT", None) != EXECUTOR:
        fail("canary candidate market prep script ROOT must bind execution-engine root")
    if getattr(prep_module, "INTEGRATION_ROOT", None) != ROOT:
        fail("canary candidate market prep script INTEGRATION_ROOT must bind integration root")
    if getattr(prep_module, "DEFAULT_GAMMA_URL", None) != "https://gamma-api.polymarket.com":
        fail("canary candidate market prep script DEFAULT_GAMMA_URL drifted")
    if getattr(prep_module, "DEFAULT_CLOB_URL", None) != "https://clob.polymarket.com":
        fail("canary candidate market prep script DEFAULT_CLOB_URL drifted")
    if getattr(prep_module, "FETCH_RETRY_ATTEMPTS", None) != 3:
        fail("canary candidate market prep script FETCH_RETRY_ATTEMPTS drifted")
    candidate_cls = getattr(prep_module, "Candidate", None)
    if candidate_cls is None or not callable(getattr(candidate_cls, "to_engine_json", None)):
        fail("canary candidate market prep script must export Candidate.to_engine_json()")
    for func_name in [
        "parse_args",
        "fetch_json",
        "fetch_json_or_error",
        "post_only_buy_limit_price",
        "candidate_from_market",
        "load_market_by_slug",
        "scan",
        "main",
    ]:
        if not callable(getattr(prep_module, func_name, None)):
            fail(f"canary candidate market prep script missing callable: {func_name}")
    candidate_json = candidate_cls(
        market_id="market-1",
        token_id="token-1",
        outcome="Yes",
        market_slug="demo-market",
        active=True,
        accepting_orders=True,
        closed=False,
        archived=False,
        best_ask=Decimal("0.42"),
        limit_price=Decimal("0.41"),
        ask_size=Decimal("25"),
        target_size=Decimal("5"),
        spread_bps=15,
        min_order_size=Decimal("1"),
        min_tick_size=Decimal("0.01"),
        liquidity_score=123,
        source_market_hash="a" * 64,
        book_snapshot_timestamp="2026-01-01T00:00:00+00:00",
        human_review_ref="https://example.invalid/review/123",
        exchange_rule_evidence_ref="https://example.invalid/rules/123",
        exchange_rule_valid_for_minutes=5,
    ).to_engine_json()
    if candidate_json.get("side") != "BUY":
        fail("canary candidate market prep candidate JSON must bind BUY side")
    if candidate_json.get("order_type") != "GTC":
        fail("canary candidate market prep candidate JSON must bind GTC order type")
    if candidate_json.get("post_only") is not True:
        fail("canary candidate market prep candidate JSON must force post_only=true")
    if candidate_json.get("human_review_ref") != "https://example.invalid/review/123":
        fail("canary candidate market prep candidate JSON must preserve human_review_ref")
    exchange_rule_snapshot = candidate_json.get("exchange_rule_snapshot")
    if not isinstance(exchange_rule_snapshot, dict):
        fail("canary candidate market prep candidate JSON missing exchange_rule_snapshot")
    for key, expected in [
        ("order_mode", "post_only_limit"),
        ("order_type", "GTC"),
        ("side", "BUY"),
        ("target_size_semantics", "outcome_shares"),
        ("evidence_ref", "https://example.invalid/rules/123"),
    ]:
        if exchange_rule_snapshot.get(key) != expected:
            fail(f"canary candidate market prep exchange_rule_snapshot must bind {key}={expected!r}")
    text = prep_script.read_text()
    if "public read-only" not in (getattr(prep_module, "__doc__", "") or ""):
        fail("canary candidate market prep script must describe public read-only sourcing")
    if "RealFundsCanaryMarketCandidate" not in (getattr(prep_module, "__doc__", "") or ""):
        fail("canary candidate market prep script must describe RealFundsCanaryMarketCandidate output shape")
    candidate_to_engine_json_body = python_function_body(text, "to_engine_json")
    parse_args_body = python_function_body(text, "parse_args")
    fetch_json_body = python_function_body(text, "fetch_json")
    fetch_json_or_error_body = python_function_body(text, "fetch_json_or_error")
    post_only_buy_limit_price_body = python_function_body(text, "post_only_buy_limit_price")
    candidate_from_market_body = python_function_body(text, "candidate_from_market")
    load_market_by_slug_body = python_function_body(text, "load_market_by_slug")
    scan_body = python_function_body(text, "scan")
    main_body = python_function_body(text, "main")
    for needle in ["candidate-market.json", "public read-only Polymarket APIs."]:
        if needle not in parse_args_body:
            fail(f"canary candidate market prep parse_args missing boundary token: {needle}")
    for needle in ["urllib.request.Request(", "urllib.request.urlopen(", "FETCH_RETRY_ATTEMPTS"]:
        if needle not in fetch_json_body:
            fail(f"canary candidate market prep fetch_json missing boundary token: {needle}")
    for needle in [
        "fetch_json(base_url, path, query, timeout_seconds)",
        "audit.setdefault(\"fetch_errors\", []).append(",
        'raise CandidateError(failure_message, audit) from exc',
    ]:
        if needle not in fetch_json_or_error_body:
            fail(f"canary candidate market prep fetch_json_or_error missing boundary token: {needle}")
    for needle in [
        "ask_ticks = (best_ask_price / min_tick_size).to_integral_value(rounding=ROUND_FLOOR)",
        "upper -= min_tick_size",
        "improved_bid = ((best_bid_price / min_tick_size).to_integral_value(rounding=ROUND_FLOOR) + 1) * min_tick_size",
        "if improved_bid < best_ask_price and improved_bid <= upper:",
        "if bid_grid > 0 and bid_grid < best_ask_price and bid_grid <= upper:",
    ]:
        if needle not in post_only_buy_limit_price_body:
            fail(f"canary candidate market prep post_only_buy_limit_price missing boundary token: {needle}")
    for needle in ["/book", "/spread", "post_only_buy_limit_price(", "selected market spread is unavailable"]:
        if needle not in candidate_from_market_body:
            fail(f"canary candidate market prep candidate_from_market missing boundary token: {needle}")
    for needle in ['fetch_json(args.gamma_url, "/markets"', 'fetch_json(args.gamma_url, "/events"']:
        if needle not in load_market_by_slug_body:
            fail(f"canary candidate market prep load_market_by_slug missing boundary token: {needle}")
    for needle in ["post_only_price_unavailable", '"remote_side_effects": False', '"authorized_for_live": False']:
        if needle not in scan_body:
            fail(f"canary candidate market prep scan missing boundary token: {needle}")
    for needle in ['"remote_side_effects": False', '"authorized_for_live": False', '"candidate_market": str(args.output)']:
        if needle not in main_body:
            fail(f"canary candidate market prep main missing audit boundary token: {needle}")
    forbidden_scan_surface = "\n".join(
        [
            getattr(prep_module, "__doc__", "") or "",
            candidate_to_engine_json_body,
            parse_args_body,
            fetch_json_body,
            fetch_json_or_error_body,
            post_only_buy_limit_price_body,
            candidate_from_market_body,
            load_market_by_slug_body,
            scan_body,
            main_body,
        ]
    )
    validate_absent_tokens(forbidden_scan_surface, "canary candidate market prep script public/safe surface", [
        "post_order",
        "post_orders",
        "private_key",
        "clob_secret",
        "api_secret",
        "POLYMARKET_PRIVATE_KEY",
        "PMX_ALLOW_LIVE_SUBMIT=1",
        "PMX_ALLOW_REAL_FUNDS_CANARY=1",
    ])


def validate_single_host_deployment_governance() -> None:
    deploy = EXECUTOR / "deploy/single-host"
    required = [
        deploy / "README.md",
        deploy / "env/pmx-api.env.example",
        deploy / "env/pmx-real-funds-canary.env.example",
        deploy / "systemd/pmx-api.service",
        deploy / "systemd/pmx-real-funds-canary@.service",
        deploy / "bin/pmx-single-host-preflight.sh",
        deploy / "bin/pmx-single-host-rollback.sh",
        deploy / "bin/pmx-single-host-canary-package-preflight.sh",
        EXECUTOR / "validation/run_single_host_deployment_drill.py",
        EXECUTOR / "validation/run_single_host_canary_candidate_drill.py",
        EXECUTOR / "validation/run_single_host_go_candidate_drill.py",
    ]
    for path in required:
        if not path.exists():
            fail(f"single-host deployment governance file missing: {path.relative_to(ROOT)}")
    readme = (deploy / "README.md").read_text()
    canary_service = (deploy / "systemd/pmx-real-funds-canary@.service").read_text()
    deployment_validator_path = EXECUTOR / "validation/run_single_host_deployment_drill.py"
    candidate_validator_path = EXECUTOR / "validation/run_single_host_canary_candidate_drill.py"
    go_candidate_validator_path = EXECUTOR / "validation/run_single_host_go_candidate_drill.py"
    validator = deployment_validator_path.read_text()
    candidate_validator = candidate_validator_path.read_text()
    go_candidate_validator = go_candidate_validator_path.read_text()
    package_preflight = (deploy / "bin/pmx-single-host-canary-package-preflight.sh").read_text()
    gate_impl = (EXECUTOR / "validation/run_current_gates_impl.sh").read_text()
    writer = (EXECUTOR / "validation/write_current_evidence_manifest.py").read_text()
    combined_templates = "\n".join(
        path.read_text()
        for path in required
        if path.exists()
        and "deploy/single-host" in path.as_posix()
        and path.name != "pmx-single-host-canary-package-preflight.sh"
    )
    for needle in [
        "single-host limited deployment",
        "not production-ready evidence",
        "PMX_LIVE_SUBMIT_ENABLED=0",
        "PMX_ALLOW_REAL_FUNDS_CANARY=0",
        "long-running HTTP listener",
        "non-live API smoke",
        "pass://polymarket-execution-engine/controlled-canary",
        "reviewed `go` release decision",
    ]:
        if needle not in readme:
            fail(f"single-host deployment README missing token: {needle}")
    if "--dry-run" not in canary_service:
        fail("single-host canary service must run dry-run mode")
    validate_absent_tokens(
        canary_service,
        "single-host canary service",
        ["--armed", "--allow-live-submit-config", "--allow-real-funds-canary-config"],
    )
    deployment_module = import_module_from_path(
        "pmx_run_single_host_deployment_drill", deployment_validator_path
    )
    if getattr(deployment_module, "DEPLOY", None) != deploy:
        fail("single-host deployment validator must bind deploy/single-host root")
    if getattr(deployment_module, "README", None) != deploy / "README.md":
        fail("single-host deployment validator must bind README.md")
    if getattr(deployment_module, "API_ENV", None) != deploy / "env/pmx-api.env.example":
        fail("single-host deployment validator must bind pmx-api env example")
    if getattr(deployment_module, "CANARY_ENV", None) != deploy / "env/pmx-real-funds-canary.env.example":
        fail("single-host deployment validator must bind canary env example")
    if getattr(deployment_module, "API_SERVICE", None) != deploy / "systemd/pmx-api.service":
        fail("single-host deployment validator must bind pmx-api service")
    if getattr(deployment_module, "CANARY_SERVICE", None) != deploy / "systemd/pmx-real-funds-canary@.service":
        fail("single-host deployment validator must bind canary service")
    if getattr(deployment_module, "PREFLIGHT", None) != deploy / "bin/pmx-single-host-preflight.sh":
        fail("single-host deployment validator must bind preflight script")
    if getattr(deployment_module, "ROLLBACK", None) != deploy / "bin/pmx-single-host-rollback.sh":
        fail("single-host deployment validator must bind rollback script")
    if getattr(deployment_module, "CANARY_PACKAGE_PREFLIGHT", None) != deploy / "bin/pmx-single-host-canary-package-preflight.sh":
        fail("single-host deployment validator must bind canary package preflight script")
    if getattr(deployment_module, "MANIFEST_WRITER", None) != EXECUTOR / "validation/write_current_evidence_manifest.py":
        fail("single-host deployment validator must bind manifest writer")
    if "PMX_ALLOW_REAL_FUNDS_CANARY=0" not in getattr(
        deployment_module, "FAIL_CLOSED_FLAGS", []
    ):
        fail("single-host deployment validator FAIL_CLOSED_FLAGS drifted")
    if "PMX_PRODUCTION_DEPLOYMENT_ENABLED=1" not in getattr(
        deployment_module, "FORBIDDEN_VALUE_FRAGMENTS", []
    ):
        fail("single-host deployment validator FORBIDDEN_VALUE_FRAGMENTS drifted")
    for fn_name in ["run_api_bind_smoke", "read", "main"]:
        if not callable(getattr(deployment_module, fn_name, None)):
            fail(f"single-host deployment validator missing callable: {fn_name}")

    candidate_module = import_module_from_path(
        "pmx_run_single_host_canary_candidate_drill", candidate_validator_path
    )
    if getattr(candidate_module, "CANARY_SERVICE", None) != deploy / "systemd/pmx-real-funds-canary@.service":
        fail("single-host canary candidate validator must bind canary service")
    if getattr(candidate_module, "MANIFEST_WRITER", None) != EXECUTOR / "validation/write_current_evidence_manifest.py":
        fail("single-host canary candidate validator must bind manifest writer")
    if not callable(getattr(candidate_module, "main", None)):
        fail("single-host canary candidate validator missing main()")

    go_candidate_module = import_module_from_path(
        "pmx_run_single_host_go_candidate_drill", go_candidate_validator_path
    )
    if getattr(go_candidate_module, "MANIFEST_WRITER", None) != EXECUTOR / "validation/write_current_evidence_manifest.py":
        fail("single-host go candidate validator must bind manifest writer")
    if not callable(getattr(go_candidate_module, "main", None)):
        fail("single-host go candidate validator missing main()")

    writer_module = import_module_from_path(
        "pmx_write_current_evidence_manifest_single_host", EXECUTOR / "validation/write_current_evidence_manifest.py"
    )
    sections = getattr(writer_module, "SECTIONS", {})
    if "single_host_deployment_validation" not in sections:
        fail("current evidence manifest writer must include single_host_deployment_validation section")
    if "single_host_canary_candidate_validation" not in sections:
        fail("current evidence manifest writer must include single_host_canary_candidate_validation section")
    if "single_host_go_candidate_validation" not in sections:
        fail("current evidence manifest writer must include single_host_go_candidate_validation section")
    for needle in [
        "single_host_deployment_validation",
        "69-single-host-deployment-drill.log",
        "live_submit_allowed",
        "production_deployment_allowed",
        "secrets_included",
        "api_bind_smoke",
        "run_api_bind_smoke",
        "PMX_PRODUCTION_DEPLOYMENT_ENABLED=1",
    ]:
        if needle not in validator:
            fail(f"single-host deployment validator missing token: {needle}")
    if "69-single-host-deployment-drill.log" not in gate_impl:
        fail("current gates must emit single-host deployment drill log")
    if "70-single-host-canary-candidate-drill.log" not in gate_impl:
        fail("current gates must emit single-host canary candidate drill log")
    if "71-single-host-go-candidate-drill.log" not in gate_impl:
        fail("current gates must emit single-host go candidate drill log")
    if '"single_host_deployment_validation"' not in writer or "69-single-host-deployment-drill.log" not in writer:
        fail("current evidence manifest writer must include single-host deployment validation")
    if '"single_host_canary_candidate_validation"' not in writer or "70-single-host-canary-candidate-drill.log" not in writer:
        fail("current evidence manifest writer must include single-host canary candidate validation")
    if '"single_host_go_candidate_validation"' not in writer or "71-single-host-go-candidate-drill.log" not in writer:
        fail("current evidence manifest writer must include single-host go candidate validation")
    for needle in [
        "candidate_package_generated",
        "release_decision",
        "no_go",
        "PMX_EXECUTION_ENGINE_ROOT",
        "single_host_canary_candidate_validation",
        "70-single-host-canary-candidate-drill.log",
    ]:
        if needle not in candidate_validator:
            fail(f"single-host canary candidate validator missing token: {needle}")
    for needle in [
        "validate_controlled_canary_external_references.py",
        "single-host canary package preflight only accepts no_go release decisions",
        "candidate-market.json",
        "market_candidate_sha256",
        "target_size",
        "release decision must keep",
        "single-host canary package preflight passed",
    ]:
        if needle not in package_preflight:
            fail(f"single-host canary package preflight missing token: {needle}")
    for needle in [
        "temporary_go_candidate_generated",
        "go_candidate_committed",
        "candidate_go_not_committed",
        "missing_release_decision_blocks_armed",
        "--release-decision-file is required with --armed",
        "FORBIDDEN_GO_DECISION_GLOBS",
        "single_host_go_candidate_validation",
        "71-single-host-go-candidate-drill.log",
    ]:
        if needle not in go_candidate_validator:
            fail(f"single-host go candidate validator missing token: {needle}")
    validate_absent_tokens(combined_templates, "single-host deployment files", [
        "-----BEGIN",
        "clob_secret=",
        "raw_signature=",
        "raw_signed_payload=",
        "signed_order_envelope=",
        "PMX_ALLOW_LIVE_SUBMIT=1",
        "PMX_ALLOW_LIVE_CANCEL=1",
        "PMX_ALLOW_REAL_FUNDS_CANARY=1",
        "PMX_PRODUCTION_DEPLOYMENT_ENABLED=1",
    ])


def validate_v28_production_live_candidate_guard() -> None:
    guard = ROOT / "scripts/check_v28_production_live_candidate.py"
    test = ROOT / "tests/test_v28_production_live_candidate.py"
    readme = ROOT / "README.md"
    report = ROOT / "VALIDATION_REPORT.md"
    for path in [guard, test]:
        if not path.exists():
            fail(f"v0.28 production-live-candidate guard file missing: {path.relative_to(ROOT)}")
    guard_text = guard.read_text()
    guard_module = import_module_from_path("pmx_check_v28_production_live_candidate", guard)
    if getattr(guard_module, "ROOT", None) != ROOT:
        fail("v0.28 production-live-candidate guard must bind integration root")
    if getattr(guard_module, "TARGET_VERSION", None) != "0.28.0":
        fail('v0.28 production-live-candidate guard TARGET_VERSION must remain "0.28.0"')
    hex64 = getattr(guard_module, "HEX64", None)
    if getattr(hex64, "pattern", None) != r"^[0-9a-f]{64}$":
        fail("v0.28 production-live-candidate guard HEX64 must enforce lowercase sha256")
    required_terms = getattr(guard_module, "REQUIRED_CANDIDATE_TERMS", None)
    if not isinstance(required_terms, list):
        fail("v0.28 production-live-candidate guard must export REQUIRED_CANDIDATE_TERMS")
    for term in [
        "production-live-candidate",
        "validated_release=false",
        "production_ready=false",
        "live_trading_ready=false",
        "operator approval",
        "runtime state healthy",
        "kill switch open",
        "no geoblock",
        "idempotency reservation",
        "rollback",
        "incident",
        "alert",
        "custody",
    ]:
        if term not in required_terms:
            fail(f"v0.28 production-live-candidate guard REQUIRED_CANDIDATE_TERMS missing {term}")
    for func_name in [
        "read_text",
        "load_json",
        "component_matrix_versions",
        "require_contains",
        "require_false",
        "evaluate",
        "main",
    ]:
        if not callable(getattr(guard_module, func_name, None)):
            fail(f"v0.28 production-live-candidate guard missing callable: {func_name}")
    if "Audit v0.28 production-live-candidate readiness." not in (getattr(guard_module, "__doc__", "") or ""):
        fail("v0.28 production-live-candidate guard must describe audit-only readiness scope")

    matrix_body = python_function_body(guard_text, "component_matrix_versions")
    for needle in ["Integration suite", "Execution engine", "Hermes adapter", 'versions["suite"]', 'versions["engine"]', 'versions["adapter"]']:
        if needle not in matrix_body:
            fail(f"v0.28 production-live-candidate guard component_matrix_versions missing token: {needle}")

    evaluate_body = python_function_body(guard_text, "evaluate")
    for needle in [
        "external_requirements = [",
        'root / "VERSION"',
        'root / "COMPONENT_COMPATIBILITY.md"',
        'root / "polymarket-execution-engine/release/manifest.json"',
        'root / "polymarket-execution-engine/evidence/current/manifest.json"',
        'root / "dist/INDEX.json"',
        'artifact.with_suffix(artifact.suffix + ".evidence.json")',
        "workspace_manifest_snapshot_path",
        'for key in ["validated_release", "production_ready", "live_trading_ready"]',
        '"production-live-candidate"',
        '"not-production"',
        '"not-live"',
    ]:
        if needle not in evaluate_body:
            fail(f"v0.28 production-live-candidate guard evaluate missing token: {needle}")

    main_body = python_function_body(guard_text, "main")
    for needle in [
        '--require-ready',
        "evaluate(ROOT)",
        "json.dumps(report, indent=2, sort_keys=True)",
        'if args.require_ready and report["status"] != "ready":',
        "return 1",
    ]:
        if needle not in main_body:
            fail(f"v0.28 production-live-candidate guard main missing token: {needle}")

    test_text = test.read_text()
    for test_name in [
        "test_ready_tree_passes_when_candidate_boundary_is_explicit",
        "test_live_ready_claim_blocks_candidate",
        "test_missing_operator_and_runtime_terms_block_candidate",
    ]:
        if test_name not in test_text:
            fail(f"v0.28 production-live-candidate tests missing function: {test_name}")
    ready_test_body = python_function_body(test_text, "test_ready_tree_passes_when_candidate_boundary_is_explicit")
    for needle in [
        "report = self.module.evaluate(self.root)",
        'self.assertEqual(report["status"], "ready")',
        'self.assertEqual(report["blockers"], [])',
        'self.assertEqual(report["external_evidence"]["status"], "not_locally_verifiable")',
    ]:
        if needle not in ready_test_body:
            fail(f"v0.28 production-live-candidate ready-tree test missing token: {needle}")
    live_ready_body = python_function_body(test_text, "test_live_ready_claim_blocks_candidate")
    for needle in [
        'manifest["release_decision"]["live_trading_ready"] = True',
        'self.assertEqual(report["status"], "not_ready")',
        'self.assertIn("live_trading_ready", "\\n".join(report["blockers"]))',
    ]:
        if needle not in live_ready_body:
            fail(f"v0.28 production-live-candidate live-ready test missing token: {needle}")
    missing_terms_body = python_function_body(test_text, "test_missing_operator_and_runtime_terms_block_candidate")
    for needle in [
        'self.assertEqual(report["status"], "not_ready")',
        'self.assertIn("operator approval", blockers)',
        'self.assertIn("runtime state healthy", blockers)',
    ]:
        if needle not in missing_terms_body:
            fail(f"v0.28 production-live-candidate missing-terms test missing token: {needle}")
    for path in [readme, report]:
        text = path.read_text()
        if "check_v28_production_live_candidate.py" not in text:
            fail(f"{path.name} must mention check_v28_production_live_candidate.py")
