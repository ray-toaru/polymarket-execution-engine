from __future__ import annotations

import inspect
import json
import re
import sys
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
    docs_text = docs_guard.read_text()
    for needle in [
        "package_release.py",
        "check_release_artifact.py",
        "release_policy.py",
        "validate_packaging_scripts",
    ]:
        if needle not in docs_text:
            fail(f"docs/evidence governance guard missing release governance token: {needle}")
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
    validator_text = validator.read_text()
    for needle in [
        "invalid partial fixture must be rejected",
        "invalid mismatched fixture must be rejected",
        "go decision missing external references",
        "go decision is expired",
        "artifact hash does not match",
        "market candidate hash does not match",
        "live_submit_authorized",
        "real_funds_canary_authorized",
        "reviewed_release_decision_present",
    ]:
        if needle not in validator_text:
            fail(f"controlled canary release-decision validator missing token: {needle}")
    review_text = review_script.read_text()
    if "release-decision.json" not in review_text or "DEFAULT_RELEASE_DECISION" not in review_text:
        fail("real-funds canary review package must include release-decision.json")
    if "external-references.json" not in review_text or "DEFAULT_EXTERNAL_REFERENCES" not in review_text:
        fail("real-funds canary review package must include external-references.json")
    for needle in [
        "--external-references-file",
        "--artifact-sha256",
        "--evidence-manifest-sha256",
        "release sidecar binds the final zip hash",
        "external_references_placeholders_remaining",
        "validate_external_references_shape",
    ]:
        if needle not in review_text:
            fail(f"real-funds canary review package missing external-reference candidate support token: {needle}")
    for needle in [
        "DEFAULT_ROOT_CI_RUN_ID",
        "DEFAULT_HERMES_CI_RUN_ID",
        "DEFAULT_EXECUTION_ENGINE_CI_RUN_ID",
        "DEFAULT_CREDENTIALED_SDK_RUN_ID",
    ]:
        if needle not in review_text:
            fail(f"real-funds canary review package must bind GitHub evidence run id token: {needle}")
    drill_text = review_drill.read_text()
    for needle in [
        "validate_controlled_canary_release_decision.py",
        "validate_controlled_canary_external_references.py",
        "credentialed_sdk_run_id",
    ]:
        if needle not in drill_text:
            fail(f"real-funds canary review package drill missing token: {needle}")
    for needle in [
        "run_real_funds_canary_blocked_rehearsal_package.py",
        "blocked real-funds canary rehearsal package failed",
    ]:
        if needle not in drill_text:
            fail(f"real-funds canary review package drill missing blocked rehearsal token: {needle}")
    for needle in ["--file", "--allow-placeholders", "must reject unresolved placeholders", "review-with-concrete-references"]:
        if needle not in drill_text:
            fail(f"real-funds canary review package drill missing external-reference candidate token: {needle}")
    external_text = external_validator.read_text()
    for needle in [
        "argparse",
        "placeholder_paths",
        "--allow-placeholders",
        "Validate an operator-supplied external reference candidate",
        "invalid sensitive fixture must be rejected",
        "forbidden sensitive reference key",
        "fixture-sensitive-value-must-not-be-logged",
        "rollback_runbook_ref",
        "incident_runbook_ref",
        "canary_retry_policy_ref",
        "references_only_no_secret_values",
    ]:
        if needle not in external_text:
            fail(f"controlled canary external-reference validator missing token: {needle}")
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
    rehearsal = EXECUTOR / "validation/run_real_funds_canary_blocked_rehearsal_package.py"
    if not rehearsal.exists():
        fail("real-funds canary blocked rehearsal package script missing")
    rehearsal_text = rehearsal.read_text()
    for needle in [
        "blocked_real_funds_canary_armed_no_go",
        "run_reviewed_go_canary_armed.py",
        "real-funds canary not allowed by release decision",
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
    text = prep_script.read_text()
    for needle in [
        "candidate-market.json",
        "public read-only",
        "urllib.request",
        "remote_side_effects",
        "authorized_for_live",
        "False",
        "/markets",
        "/book",
        "/spread",
        "max_order_notional_usd",
        "target_size",
        "estimated_order_notional_usd",
        "exchange_rule_snapshot",
        "post_only_buy_limit_price",
        "post_only_price_unavailable",
        "limit_price",
        "max_spread_bps",
        "RealFundsCanaryMarketCandidate",
        "--human-review-ref",
        "--market-url",
        "--outcome",
        "human_review_ref",
        "order_type",
    ]:
        if needle not in text:
            fail(f"canary candidate market prep script missing boundary token: {needle}")
    validate_absent_tokens(text, "canary candidate market prep script", [
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
    validator = (EXECUTOR / "validation/run_single_host_deployment_drill.py").read_text()
    candidate_validator = (EXECUTOR / "validation/run_single_host_canary_candidate_drill.py").read_text()
    go_candidate_validator = (EXECUTOR / "validation/run_single_host_go_candidate_drill.py").read_text()
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
    for needle in [
        'TARGET_VERSION = "0.28.0"',
        "production-live-candidate",
        "--require-ready",
        "release artifact evidence sidecar missing",
        "workspace manifest snapshot referenced by release artifact evidence sidecar is missing",
        "validated_release",
        "production_ready",
        "live_trading_ready",
        "operator approval",
        "runtime state healthy",
    ]:
        if needle not in guard_text:
            fail(f"v0.28 production-live-candidate guard missing token: {needle}")
    test_text = test.read_text()
    for needle in [
        "test_ready_tree_passes_when_candidate_boundary_is_explicit",
        "test_live_ready_claim_blocks_candidate",
        "test_missing_operator_and_runtime_terms_block_candidate",
    ]:
        if needle not in test_text:
            fail(f"v0.28 production-live-candidate tests missing token: {needle}")
    for path in [readme, report]:
        text = path.read_text()
        if "check_v28_production_live_candidate.py" not in text:
            fail(f"{path.name} must mention check_v28_production_live_candidate.py")
