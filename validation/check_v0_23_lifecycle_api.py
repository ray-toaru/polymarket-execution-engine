#!/usr/bin/env python3
"""Static guard for v0.23 lifecycle/query API and runtime-state aggregation scaffolding."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "crates" / "pmx-api" / "src" / "lib.rs"
API_FAKE_E2E = ROOT / "crates" / "pmx-api" / "tests" / "http_and_fake_e2e.rs"
API_PG_E2E = ROOT / "crates" / "pmx-api" / "tests" / "http_postgres_e2e.rs"
AUTHZ = ROOT / "crates" / "pmx-authz" / "src" / "lib.rs"
SERVICE = ROOT / "crates" / "pmx-service" / "src" / "lib.rs"
STORE = ROOT / "crates" / "pmx-store" / "src" / "lib.rs"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
GATEWAY = ROOT / "crates" / "pmx-gateway" / "src" / "lib.rs"
OPENAPI = ROOT / "openapi" / "executor.v1.yaml"
GATE = ROOT / "validation" / "run_v0_23_gates.sh"
SOURCE_CANDIDATE_DOC = ROOT / "docs" / "V0_23_SOURCE_CANDIDATE.md"
VERSION_GUARD = ROOT.parent / "scripts" / "check_version_consistency.py"
HERMES_CLIENT = ROOT.parent / "hermes-polymarket-control" / "src" / "hermes_polymarket_control" / "client.py"
HERMES_MODELS = ROOT.parent / "hermes-polymarket-control" / "src" / "hermes_polymarket_control" / "models.py"
EVIDENCE_MANIFEST = ROOT / "validation" / "templates" / "evidence_manifest.template.json"
CURRENT_EVIDENCE_MANIFEST = ROOT / "evidence" / "current" / "manifest.json"
EVIDENCE_GUARD = ROOT / "validation" / "check_v0_23_evidence_manifest.py"
GOVERNANCE_GUARD = ROOT / "validation" / "check_docs_evidence_governance.py"
EVIDENCE_WRITER = ROOT / "validation" / "write_v0_23_evidence_manifest.py"

REQUIRED = {
    API: [
        "/v1/sign-only/lifecycle-events",
        "/v1/sign-only/lifecycle-events/:execution_id",
        "/v1/lifecycle/executions/:execution_id/events",
        "/v1/admin/audit-events",
        "record_sign_only_lifecycle_event",
        "list_sign_only_lifecycle_events",
        "list_execution_lifecycle_events",
        "list_admin_audit_events",
        "before_event_id",
        "before_audit_id",
        "operation: query.operation",
        "principal_subject: query.principal_subject",
        "result: query.result",
        "CANCEL_REQUESTED_NON_LIVE",
        "RECONCILE_REQUESTED_NON_LIVE",
        "correlation_id_from_headers",
        "api_error_with_correlation",
        "redacted_payload_envelope",
    ],
    AUTHZ: [
        "ReadAudit",
        "RecordSignOnlyLifecycle",
        "Operation::ReadAudit",
        "Operation::RecordSignOnlyLifecycle",
    ],
    ROOT / "crates" / "pmx-core" / "src" / "lib.rs": [
        "RedactedPayloadEnvelope",
        "redacted_fields",
        "redacted_payload_envelope",
        "signed_payload",
    ],
    SERVICE: [
        "validate_sign_only_lifecycle_append",
        "sign_only_lifecycle_records_equivalent",
        "SignOnlyLifecycleQuery",
        "record_sign_only_lifecycle_event",
        "list_sign_only_lifecycle_events",
        "list_admin_audit_events",
        "list_execution_lifecycle_events",
        "service_validates_and_persists_sign_only_lifecycle_sequence",
    ],
    STORE: [
        "OrderLifecycleRecord",
        "OrderLifecycleStore",
        "record_order_lifecycle_event",
        "in_memory_order_lifecycle_records_cancel_requested",
        "AdminAuditQuery",
        "ExecutionLifecycleQuery",
        "SignOnlyLifecycleQuery",
        "RUNTIME_OBSERVATION_TTL_SECONDS",
        "PMX_RUNTIME_OBSERVATION_TTL_SECONDS",
        "sign_only_lifecycle_record_is_replay",
        "validate_sign_only_lifecycle_append_for_store",
        "sanitize_sign_only_lifecycle_record",
        "list_admin_audit_events",
        "list_execution_lifecycle_events",
        "apply_runtime_worker_observations",
        "runtime_worker_observations_degrade_loaded_runtime_state",
        "RuntimeWorkerHeartbeat",
        "RuntimeWorkerHealthStore",
        "in_memory_worker_heartbeat_informs_runtime_state",
    ],
    POSTGRES: [
        "impl OrderLifecycleStore for PostgresStore",
        "postgres_records_order_lifecycle_event",
        "DISTINCT ON (capability)",
        "pg_advisory_xact_lock",
        "sign_only_lifecycle",
        "FOREIGN_KEY_VIOLATION",
        "CHECK_VIOLATION",
        "before_event_id",
        "before_audit_id",
        "principal_subject = $4",
        "result = $5",
        "observed_at >= now()",
        "runtime_observation_ttl_seconds",
        "apply_runtime_worker_observations",
        "postgres_runtime_worker_observations_degrade_runtime_state",
        "postgres_records_cancel_reconcile_lifecycle_events",
        "impl RuntimeWorkerHealthStore for PostgresStore",
        "postgres_records_worker_heartbeat",
    ],
    API_FAKE_E2E: [
        "/v1/sign-only/lifecycle-events",
        "/v1/lifecycle/executions/",
        "/v1/admin/audit-events?limit=20",
        "CANCEL_REQUESTED_NON_LIVE",
        "RECONCILE_REQUESTED_NON_LIVE",
        "schema_version",
        "correlation_id",
        "redacted_fields",
        "event[\"payload\"][\"correlation_id\"].as_str().is_some()",
    ],
    API_PG_E2E: [
        "sign-only PG lifecycle response",
        "PG lifecycle events",
        "degraded snapshot response",
        "audit query response",
        "schema_version",
        "correlation_id",
        "redacted_fields",
    ],
    GATEWAY: [
        "fake_gateway_cancel_maps_to_order_lifecycle_state_machine",
        "transition_order_state",
        "OrderEventKind::CancelRemoteAccepted",
    ],
    ROOT / "migrations" / "0001_initial.sql": [
        "CREATE TABLE IF NOT EXISTS orders",
        "CREATE TABLE IF NOT EXISTS order_events",
        "idx_order_events_order_created",
        "ADD COLUMN IF NOT EXISTS client_event_id",
        "ADD COLUMN IF NOT EXISTS observed_at",
        "ADD COLUMN IF NOT EXISTS correlation_id",
    ],
    OPENAPI: [
        "/v1/sign-only/lifecycle-events",
        "/v1/sign-only/lifecycle-events/{execution_id}",
        "/v1/lifecycle/executions/{execution_id}/events",
        "/v1/admin/audit-events",
        "before_event_id",
        "before_audit_id",
        "principal_subject",
        "result",
        "readOnly: true",
        "created_at",
        "client_event_id",
        "SignOnlyLifecycleRecord",
        "RedactedPayloadEnvelope",
        "payload: { $ref: '#/components/schemas/RedactedPayloadEnvelope' }",
        "ExecutionLifecycleEvent",
        "AdminAuditEvent",
    ],
    GATE: [
        "run_current_gates.sh",
        "check_v0_23_lifecycle_api.py",
        "check_version_consistency.py",
        "check_v0_23_evidence_manifest.py",
        "check_docs_evidence_governance.py",
        "write_v0_23_evidence_manifest.py",
        "evidence/current",
        "v0.23 gates completed",
    ],
    SOURCE_CANDIDATE_DOC: [
        "v0.23.0 source candidate",
        "not confirmed here",
        "live submit",
    ],
    VERSION_GUARD: [
        "version consistency passed",
        "run_current_gates.sh",
        "run_v0_23_gates.sh",
        "source-candidate",
    ],
    HERMES_CLIENT: [
        "record_sign_only_lifecycle_event",
        "list_sign_only_lifecycle_events",
        "list_execution_lifecycle_events",
        "list_admin_audit_events",
        "principal_subject: str | None = None",
        "result: str | None = None",
        "execution_id: str | None = None",
        "X-Correlation-Id",
    ],
    HERMES_MODELS: [
        "class SignOnlyLifecycleRecord",
        "client_event_id",
        "class RedactedPayloadEnvelope",
        "payload: RedactedPayloadEnvelope",
        "class ExecutionLifecycleEvent",
        "class AdminAuditEvent",
    ],
    EVIDENCE_MANIFEST: [
        "canonical_evidence_dir",
        "rust_workspace_validation",
        "postgres_validation",
        "credentialed_non_trading_validation",
        "validated_release",
    ],
    CURRENT_EVIDENCE_MANIFEST: [
        "canonical_evidence_dir",
        "local_static_validation",
        "release_decision",
    ],
    EVIDENCE_GUARD: [
        "validated_release=true",
        "non-pass evidence sections",
        "artifact.sha256",
        "v0.23 evidence manifest guard passed",
    ],
    GOVERNANCE_GUARD: [
        "docs/evidence governance guard passed",
        "canonical evidence manifest",
        "archive-excluded-from-release-package",
    ],
    EVIDENCE_WRITER: [
        "generated_from_gate_logs",
        "canonical_evidence_dir",
        "artifact",
        "sha256",
    ],
}

FORBIDDEN = {
    API: [
        "SignedOrderEnvelope",
        "post_order(",
        "submit_live",
    ],
    OPENAPI: [
        "SignedOrderEnvelope",
        "signed_payload",
        "private_key",
        "clob_api_secret",
    ],
}


def main() -> int:
    failures: list[str] = []
    for path, needles in REQUIRED.items():
        text = path.read_text()
        for needle in needles:
            if needle not in text:
                failures.append(f"{path.relative_to(ROOT)} missing {needle}")
    for path, needles in FORBIDDEN.items():
        text = path.read_text()
        for needle in needles:
            if needle in text:
                failures.append(f"{path.relative_to(ROOT)} contains forbidden token {needle}")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("v0.23 lifecycle/query static guard passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
