from __future__ import annotations

import re
from collections.abc import Iterator

from validate_contracts_support import (
    API_SRC,
    CONTROL,
    CORE_SRC,
    EXPECTED_202_PATHS,
    FORBIDDEN_PUBLIC_TOKEN_PATTERNS,
    PUBLIC_CONTRACT_SOURCE_PATHS,
    PY_MODEL_BY_SCHEMA,
    SQL,
    fail,
    import_control_models,
    rust_handler_body,
    rust_routes,
    rust_source_text,
)
from validate_contracts_executor import (
    operation_request_ref,
    operation_response_ref,
    schema_property_names,
    schema_required_names,
)


def iter_json_strings(value: object) -> Iterator[str]:
    if isinstance(value, str):
        yield value
        return
    if isinstance(value, dict):
        for key, nested in value.items():
            yield str(key)
            yield from iter_json_strings(nested)
        return
    if isinstance(value, list):
        for nested in value:
            yield from iter_json_strings(nested)


def split_top_level_csv(text: str) -> list[str]:
    parts: list[str] = []
    current: list[str] = []
    depth = 0
    for char in text:
        if char == "(":
            depth += 1
        elif char == ")" and depth > 0:
            depth -= 1
        if char == "," and depth == 0:
            item = "".join(current).strip()
            if item:
                parts.append(item)
            current = []
            continue
        current.append(char)
    tail = "".join(current).strip()
    if tail:
        parts.append(tail)
    return parts


def rust_struct_has_deny_unknown_fields(text: str, struct_name: str) -> bool:
    pattern = re.compile(
        rf"(?P<attrs>(?:\s*#\[[^\]]+\]\s*)*)"
        rf"(?P<vis>pub(?:\([^)]*\))?\s+)?struct\s+{re.escape(struct_name)}\b"
    )
    match = pattern.search(text)
    if not match:
        return False
    attrs = match.group("attrs")
    return any(
        "serde" in attr and "deny_unknown_fields" in attr
        for attr in re.findall(r"#\[[^\]]+\]", attrs)
    )


def validate_paths_and_statuses(spec: dict) -> None:
    openapi_paths = set(spec["paths"].keys())
    routes = rust_routes()
    missing = openapi_paths - routes
    extra = routes - openapi_paths
    if missing:
        fail(f"Rust routes missing OpenAPI paths: {sorted(missing)}")
    if extra:
        fail(f"Rust routes not present in OpenAPI: {sorted(extra)}")
    for path, handler in EXPECTED_202_PATHS.items():
        operations = spec["paths"][path]
        statuses = {
            status
            for operation in operations.values()
            for status in operation.get("responses", {}).keys()
        }
        if "202" not in statuses:
            fail(f"OpenAPI path {path} does not declare 202")
        body = rust_handler_body(handler)
        if "StatusCode::ACCEPTED" not in body:
            fail(f"handler {handler} does not visibly return StatusCode::ACCEPTED")


def validate_critical_contract_shapes(spec: dict) -> None:
    expected_refs = {
        ("/v1/submissions", "post", "request"): "#/components/schemas/SubmitRequest",
        ("/v1/submissions", "post", "202"): "#/components/schemas/SubmitReceipt",
        ("/v1/admin/kill-switch", "post", "request"): "#/components/schemas/KillSwitchRequest",
        ("/v1/admin/kill-switch", "post", "202"): "#/components/schemas/KillSwitchReceipt",
        ("/v1/admin/cancel-order", "post", "request"): "#/components/schemas/CancelOrderRequest",
        ("/v1/admin/cancel-order", "post", "202"): "#/components/schemas/CancelReceipt",
        ("/v1/admin/reconcile", "post", "request"): "#/components/schemas/ReconcileRequest",
        ("/v1/admin/reconcile", "post", "202"): "#/components/schemas/ReconcileReport",
    }
    for (path, method, kind), expected_ref in expected_refs.items():
        actual_ref = operation_request_ref(spec, path, method) if kind == "request" else operation_response_ref(spec, path, method, kind)
        if actual_ref != expected_ref:
            fail(f"critical contract {method.upper()} {path} {kind} must reference {expected_ref}, got {actual_ref}")

    exact_required = {
        "SubmitRequest": {"execution_id", "plan_hash", "idempotency_key", "mode"},
        "SubmitReceipt": {"execution_id", "receipt_id", "status", "executor_version", "contract_version"},
        "KillSwitchRequest": {"scope", "enabled", "reason"},
        "KillSwitchReceipt": {"scope", "enabled", "changed_at", "effective_at", "state_version", "persisted", "reason"},
        "CancelOrderRequest": {"account_id", "order_id", "reason"},
        "CancelReceipt": {"cancel_id", "order_id", "state"},
        "ReconcileRequest": {"account_id", "execution_id", "reason"},
        "ReconcileReport": {"reconcile_id", "status", "checked_orders", "findings"},
    }
    exact_props = {
        "SubmitRequest": {"execution_id", "plan_hash", "idempotency_key", "mode"},
        "SubmitReceipt": {"execution_id", "receipt_id", "status", "executor_version", "contract_version"},
        "KillSwitchRequest": {"scope", "account_id", "enabled", "reason"},
        "KillSwitchReceipt": {"scope", "account_id", "enabled", "changed_at", "effective_at", "state_version", "persisted", "reason"},
        "CancelOrderRequest": {"account_id", "execution_id", "order_id", "reason"},
        "CancelReceipt": {"cancel_id", "order_id", "state"},
        "ReconcileRequest": {"account_id", "execution_id", "order_id", "reason", "remote_observation"},
        "ReconcileReport": {"reconcile_id", "status", "checked_orders", "findings"},
    }
    for schema_name, expected in exact_required.items():
        actual = schema_required_names(spec, schema_name)
        if actual != expected:
            fail(f"critical contract schema {schema_name} required fields changed: expected {sorted(expected)} got {sorted(actual)}")
    for schema_name, expected in exact_props.items():
        actual = schema_property_names(spec, schema_name)
        if actual != expected:
            fail(f"critical contract schema {schema_name} properties changed: expected {sorted(expected)} got {sorted(actual)}")


def validate_no_public_forbidden_tokens(spec: dict) -> None:
    public_strings = list(iter_json_strings(spec))
    for token_name, pattern in FORBIDDEN_PUBLIC_TOKEN_PATTERNS.items():
        if any(pattern.search(value) for value in public_strings):
            fail(f"forbidden token in public OpenAPI: {token_name}")

    for path in PUBLIC_CONTRACT_SOURCE_PATHS:
        candidates = sorted(path.rglob("*.py")) if path.is_dir() else [path]
        for candidate in candidates:
            text = candidate.read_text()
            for token_name, pattern in FORBIDDEN_PUBLIC_TOKEN_PATTERNS.items():
                if pattern.search(text):
                    fail(
                        f"forbidden token {token_name} in public contract source {candidate.relative_to(CONTROL.parent)}"
                    )


def validate_additional_properties(spec: dict) -> None:
    for name, schema in spec["components"]["schemas"].items():
        if schema.get("type") == "object" and name != "HealthReport":
            if schema.get("additionalProperties") is not False:
                fail(f"schema {name} must set additionalProperties: false")
        if name == "HealthReport" and schema.get("additionalProperties") is not False:
            fail("HealthReport must set additionalProperties: false at top level")


def validate_python_field_parity(spec: dict) -> None:
    models = import_control_models()
    for schema_name, model_name in PY_MODEL_BY_SCHEMA.items():
        schema = spec["components"]["schemas"][schema_name]
        props = set(schema.get("properties", {}).keys())
        py_model = getattr(models, model_name)
        py_fields = set(py_model.model_fields.keys())
        if props != py_fields:
            fail(f"Python model {model_name} fields {sorted(py_fields)} != OpenAPI {schema_name} fields {sorted(props)}")
        py_schema = py_model.model_json_schema()
        schema_required = set(schema.get("required", []))
        py_required = set(py_schema.get("required", []))
        if schema_required != py_required:
            fail(
                f"Python model {model_name} required fields {sorted(py_required)} != OpenAPI {schema_name} required fields {sorted(schema_required)}"
            )
        if schema.get("additionalProperties") != py_schema.get("additionalProperties"):
            fail(
                f"Python model {model_name} additionalProperties {py_schema.get('additionalProperties')} != OpenAPI {schema_name} additionalProperties {schema.get('additionalProperties')}"
            )


def validate_sql_idempotency() -> None:
    sql = SQL.read_text()
    table_match = re.search(
        r"CREATE TABLE IF NOT EXISTS idempotency_records\s*\((?P<body>.*?)\);\s*",
        sql,
        re.DOTALL,
    )
    if not table_match:
        fail("SQL missing idempotency_records table")
    entries = split_top_level_csv(table_match.group("body"))
    normalized_entries = [" ".join(entry.split()) for entry in entries]

    for entry in normalized_entries:
        if re.match(r"idempotency_key\s+TEXT\s+PRIMARY\s+KEY\b", entry):
            fail("idempotency_key must not be a global primary key")

    if not any(re.match(r"request_fingerprint\s+TEXT\s+NOT\s+NULL\b", entry) for entry in normalized_entries):
        fail("SQL missing idempotency invariant: request_fingerprint TEXT NOT NULL")
    if not any(
        re.match(
            r"submit_attempt\s+INTEGER\s+NOT\s+NULL\s+CHECK\s*\(\s*submit_attempt\s*>=\s*1\s*\)$",
            entry,
        )
        for entry in normalized_entries
    ):
        fail("SQL missing idempotency invariant: submit_attempt INTEGER NOT NULL CHECK (submit_attempt >= 1)")

    unique_constraints = set()
    for entry in normalized_entries:
        match = re.match(r"UNIQUE\s*\((?P<fields>[^)]+)\)", entry)
        if not match:
            continue
        unique_constraints.add(tuple(field.strip() for field in match.group("fields").split(",")))

    for fields in [
        ("account_id", "execution_id", "idempotency_key"),
        ("account_id", "execution_id", "submit_attempt"),
    ]:
        if fields not in unique_constraints:
            fail(f"SQL missing idempotency invariant: UNIQUE({', '.join(fields)})")


def validate_rust_deny_unknown_fields() -> None:
    file_structs = {
        API_SRC / "model.rs": [
            "DecisionRequest",
            "CompilePlanRequest",
            "SubmitPlanRequest",
            "CancelOrderRequest",
            "ReconcileOrderLocalRequest",
        ],
        CORE_SRC / "domain/intent.rs": ["TradeIntent", "NormalizedIntent"],
        CORE_SRC / "domain/plan/ops.rs": ["KillSwitchRequest", "ReconcileRequest"],
    }
    for path, struct_names in file_structs.items():
        text = path.read_text()
        for struct_name in struct_names:
            if not rust_struct_has_deny_unknown_fields(text, struct_name):
                fail(f"Rust DTO {struct_name} missing #[serde(deny_unknown_fields)] in {path.relative_to(CONTROL.parent)}")
