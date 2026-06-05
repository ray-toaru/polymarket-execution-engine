from __future__ import annotations

import re
import tomllib

from validate_contracts_support import (
    API_E2E_TEST,
    API_POSTGRES_E2E_TEST,
    API_SRC,
    CORE_SRC,
    EXECUTOR,
    GATEWAY_SRC,
    LIVE_SUBMIT_GUARD,
    OPENAPI,
    POSTGRES_RS,
    ROOT,
    ROOT_CARGO_TOML,
    SDK_ADAPTER_RS,
    SDK_ADAPTER_SRC,
    SDK_ADAPTER_TOML,
    SDK_SPIKE_RS,
    SDK_SPIKE_TOML,
    SERVICE_RS,
    SERVICE_SRC,
    SERVICE_TOML,
    SQL,
    STORE_SRC,
    fail,
    rust_file_with_modules_text,
    rust_source_text,
)


def openapi_operation(spec: dict, path: str, method: str) -> dict:
    try:
        return spec["paths"][path][method]
    except KeyError as exc:
        fail(f"OpenAPI missing {method.upper()} {path}: {exc}")


def operation_parameter_names(operation: dict) -> set[str]:
    return {param["name"] for param in operation.get("parameters", [])}


def schema_property_names(spec: dict, schema_name: str) -> set[str]:
    try:
        schema = spec["components"]["schemas"][schema_name]
    except KeyError as exc:
        fail(f"OpenAPI missing schema {schema_name}: {exc}")
    return set(schema.get("properties", {}).keys())


def schema_required_names(spec: dict, schema_name: str) -> set[str]:
    try:
        schema = spec["components"]["schemas"][schema_name]
    except KeyError as exc:
        fail(f"OpenAPI missing schema {schema_name}: {exc}")
    return set(schema.get("required", []))


def operation_request_ref(spec: dict, path: str, method: str) -> str | None:
    return (
        openapi_operation(spec, path, method)
        .get("requestBody", {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("$ref")
    )


def operation_response_ref(spec: dict, path: str, method: str, status: str) -> str | None:
    return (
        openapi_operation(spec, path, method)
        .get("responses", {})
        .get(status, {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("$ref")
    )


def operation_response_array_item_ref(spec: dict, path: str, method: str, status: str) -> str | None:
    return (
        openapi_operation(spec, path, method)
        .get("responses", {})
        .get(status, {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("items", {})
        .get("$ref")
    )


def require_tokens(text: str, label: str, tokens: list[str]) -> None:
    for token in tokens:
        if token not in text:
            fail(f"{label} missing token: {token}")


def require_file_tokens(path, label: str, tokens: list[str]) -> None:
    if not path.exists():
        fail(f"missing {label} source: {path.relative_to(ROOT)}")
    require_tokens(path.read_text(), label, tokens)


def validate_absent_tokens(text: str, label: str, tokens: list[str]) -> None:
    for token in tokens:
        if token in text:
            fail(f"{label} contains forbidden token: {token}")


def source_tree_text_without_paths(root, excluded_paths: list) -> str:
    text = rust_source_text(root)
    for excluded in excluded_paths:
        if excluded.exists():
            text = text.replace(excluded.read_text(), "")
    return text


def validate_required_groups(groups: dict[str, tuple[str, list[str]]]) -> None:
    for label, (text, needles) in groups.items():
        require_tokens(text, label, needles)


def rust_module_names(text: str, prefix: str = "mod") -> set[str]:
    pattern = rf"(?m)^\s*{re.escape(prefix)}\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;"
    return set(re.findall(pattern, text))


def rust_pub_use_targets(text: str) -> set[str]:
    return set(re.findall(r"(?m)^\s*pub\s+use\s+([a-zA-Z_][a-zA-Z0-9_]*)::", text))


def rust_async_fn_names(text: str) -> set[str]:
    return set(
        re.findall(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?async\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)",
            text,
        )
    )


def rust_fn_names(text: str) -> set[str]:
    return set(re.findall(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(", text))


def rust_const_names(text: str) -> set[str]:
    return set(re.findall(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+([A-Z][A-Z0-9_]*)\s*:", text))


def rust_struct_field_names(text: str, struct_name: str) -> set[str]:
    pattern = rf"(?s)struct\s+{re.escape(struct_name)}[^\{{]*\{{(.*?)\n\}}"
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust struct: {struct_name}")
    body = match.group(1)
    return set(
        re.findall(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*:",
            body,
        )
    )


def rust_enum_variant_names(text: str, enum_name: str) -> set[str]:
    pattern = rf"(?s)enum\s+{re.escape(enum_name)}[^\{{]*\{{(.*?)\n\}}"
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust enum: {enum_name}")
    body = match.group(1)
    return set(
        re.findall(
            r"(?m)^\s*([A-Z][a-zA-Z0-9_]*)\s*(?:,|\(|\{|$)",
            body,
        )
    )


def rust_enum_variant_field_names(text: str, enum_name: str, variant_name: str) -> set[str]:
    pattern = rf"(?s)enum\s+{re.escape(enum_name)}[^\{{]*\{{(.*?)\n\}}"
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust enum: {enum_name}")
    body = match.group(1)
    variant_pattern = rf"(?s)\b{re.escape(variant_name)}\s*\{{(.*?)\}}"
    variant_match = re.search(variant_pattern, body)
    if not variant_match:
        fail(f"missing Rust enum variant: {enum_name}::{variant_name}")
    variant_body = variant_match.group(1)
    return set(
        re.findall(
            r"(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:",
            variant_body,
        )
    )


def ensure_match_arms(text: str, label: str, fn_name: str, required_arms: list[str]) -> None:
    marker = f"async fn {fn_name}"
    start = text.find(marker)
    if start == -1:
        fail(f"{label} missing async fn: {fn_name}")
    next_match = re.search(r"\n\s*(?:pub(?:\([^)]*\))?\s+)?async fn ", text[start + len(marker) :])
    end = len(text) if not next_match else start + len(marker) + next_match.start()
    body = text[start:end]
    for arm in required_arms:
        if arm not in body:
            fail(f"{label} missing token: {arm}")


def rust_trait_method_signatures(text: str, trait_name: str) -> set[str]:
    pattern = rf"(?s)trait\s+{re.escape(trait_name)}[^\{{]*\{{(.*?)\n\}}"
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust trait: {trait_name}")
    body = match.group(1)
    return set(
        re.findall(
            r"(?m)^\s*async\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
            body,
        )
    )


def rust_impl_block_body(text: str, impl_pattern: str, label: str) -> str:
    match = re.search(impl_pattern, text)
    if not match:
        fail(label)
    body_start = text.find("{", match.end() - 1)
    if body_start == -1:
        fail(label)
    depth = 0
    for index in range(body_start, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[body_start + 1 : index]
    fail(label)


def rust_impl_trait_method_names(text: str, trait_name: str, for_type: str) -> set[str]:
    body = rust_impl_block_body(
        text,
        rf"(?m)^\s*impl(?:<[^>]+>)?\s+{re.escape(trait_name)}\s+for\s+{re.escape(for_type)}[^\{{]*\{{",
        f"missing Rust impl: impl {trait_name} for {for_type}",
    )
    return set(
        re.findall(
            r"(?m)^\s*async\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
            body,
        )
    ) | set(
        re.findall(
            r"(?m)^\s*fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
            body,
        )
    )


def rust_impl_trait_method_body(text: str, trait_name: str, for_type: str, fn_name: str) -> str:
    body = rust_impl_block_body(
        text,
        rf"(?m)^\s*impl(?:<[^>]+>)?\s+{re.escape(trait_name)}\s+for\s+{re.escape(for_type)}[^\{{]*\{{",
        f"missing Rust impl: impl {trait_name} for {for_type}",
    )
    fn_pattern = re.compile(
        rf"(?m)^\s*(?:async\s+)?fn\s+{re.escape(fn_name)}\s*\(",
    )
    fn_match = fn_pattern.search(body)
    if not fn_match:
        fail(f"missing Rust impl method body: {trait_name} for {for_type}::{fn_name}")
    body_start = body.find("{", fn_match.end())
    if body_start == -1:
        fail(f"missing Rust impl method body: {trait_name} for {for_type}::{fn_name}")
    depth = 0
    for index in range(body_start, len(body)):
        char = body[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return body[body_start + 1 : index]
    fail(f"unterminated Rust impl method body: {trait_name} for {for_type}::{fn_name}")


def rust_inherent_impl_method_names(text: str, type_name: str) -> set[str]:
    body = rust_impl_block_body(
        text,
        rf"(?m)^\s*impl(?:<[^>]+>)?\s+{re.escape(type_name)}[^\{{]*\{{",
        f"missing Rust impl: impl {type_name}",
    )
    return set(
        re.findall(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?async\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
            body,
        )
    ) | set(
        re.findall(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
            body,
        )
    )


def rust_impl_method_body(text: str, type_name: str, fn_name: str) -> str:
    body = rust_impl_block_body(
        text,
        rf"(?m)^\s*impl(?:<[^>]+>)?\s+{re.escape(type_name)}[^\{{]*\{{",
        f"missing Rust impl: impl {type_name}",
    )
    fn_pattern = re.compile(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+{re.escape(fn_name)}\s*\(",
    )
    fn_match = fn_pattern.search(body)
    if not fn_match:
        fail(f"missing Rust impl method body: {type_name}::{fn_name}")
    body_start = body.find("{", fn_match.end())
    if body_start == -1:
        fail(f"missing Rust impl method body: {type_name}::{fn_name}")
    depth = 0
    for index in range(body_start, len(body)):
        char = body[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return body[body_start + 1 : index]
    fail(f"unterminated Rust impl method body: {type_name}::{fn_name}")


def service_backend_method_names(text: str) -> set[str]:
    if "impl ServiceBackend" not in text:
        fail("missing Rust impl: impl ServiceBackend")
    return rust_async_fn_names(text) | rust_fn_names(text)


def rust_async_fn_signature(text: str, fn_name: str) -> tuple[str, str]:
    pattern = (
        rf"(?s)async\s+fn\s+{re.escape(fn_name)}\s*\((.*?)\)\s*->\s*([^\{{]+)\{{"
    )
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust async fn signature: {fn_name}")
    params = match.group(1)
    return_type = match.group(2).strip()
    return params, return_type


def rust_fn_signature(text: str, fn_name: str) -> tuple[str, str]:
    pattern = rf"(?s)fn\s+{re.escape(fn_name)}\s*\((.*?)\)\s*->\s*([^\{{]+)\{{"
    match = re.search(pattern, text)
    if not match:
        fail(f"missing Rust fn signature: {fn_name}")
    params = match.group(1)
    return_type = match.group(2).strip()
    return params, return_type


def rust_fn_body(text: str, fn_name: str) -> str:
    fn_pattern = re.compile(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+{re.escape(fn_name)}\s*\(",
    )
    fn_match = fn_pattern.search(text)
    if not fn_match:
        fail(f"missing Rust fn body: {fn_name}")
    body_start = text.find("{", fn_match.end())
    if body_start == -1:
        fail(f"missing Rust fn body: {fn_name}")
    depth = 0
    for index in range(body_start, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[body_start + 1 : index]
    fail(f"unterminated Rust fn body: {fn_name}")


def cargo_toml(path: Path) -> dict:
    try:
        return tomllib.loads(path.read_text())
    except FileNotFoundError:
        fail(f"missing Cargo/TOML file: {path.relative_to(ROOT)}")


def validate_v04_source_landings() -> None:
    if not API_E2E_TEST.exists():
        fail("missing HTTP/auth/fake E2E integration test source")
    smoke_text = (API_E2E_TEST.parent / "http_and_fake_e2e/smoke.rs").read_text()
    for fn_name in ["http_auth_and_fake_e2e_smoke"]:
        if fn_name not in smoke_text:
            fail(f"HTTP/auth fake E2E smoke missing token: {fn_name}")
    for status in ["StatusCode::UNAUTHORIZED", "StatusCode::FORBIDDEN", "StatusCode::ACCEPTED"]:
        if status not in smoke_text:
            fail(f"HTTP/auth fake E2E smoke missing token: {status}")
    advisory_text = (STORE_SRC / "model/advisory.rs").read_text()
    if not re.search(r"pub\s+struct\s+AdvisoryLockKey\b", advisory_text):
        fail("pmx-store advisory lock model missing token: pub struct AdvisoryLockKey")
    if not re.search(r"pub\s+fn\s+advisory_lock_key\s*\(", advisory_text):
        fail("pmx-store advisory lock model missing token: pub fn advisory_lock_key")
    for needle in ["const FNV_OFFSET", "const FNV_PRIME", "i64::from_ne_bytes(hash.to_ne_bytes())"]:
        if needle not in advisory_text:
            fail(f"pmx-store advisory lock model missing token: {needle}")
    postgres_text = (STORE_SRC / "postgres.rs").read_text()
    if not re.search(r"pub\s+struct\s+PostgresStore\b", postgres_text):
        fail("pmx-store postgres adapter missing token: pub struct PostgresStore")
    if "tokio_postgres::connect(&self.database_url, NoTls)" not in postgres_text:
        fail("pmx-store postgres adapter missing token: tokio_postgres::connect(&self.database_url, NoTls)")
    if not POSTGRES_RS.exists():
        fail("missing PostgreSQL repository adapter source")
    idempotency_adapter = (STORE_SRC / "postgres_idempotency.rs").read_text()
    if "impl IdempotencyStore for PostgresStore" not in idempotency_adapter:
        fail("postgres idempotency adapter missing token: impl IdempotencyStore for PostgresStore")
    ensure_match_arms(
        idempotency_adapter.replace("async fn finish_submit_attempt", "\nasync fn finish_submit_attempt"),
        "postgres idempotency adapter",
        "begin_submit_attempt",
        ["begin::begin_submit_attempt(", "request_fingerprint"],
    )
    ensure_match_arms(
        idempotency_adapter.replace("async fn finish_submit_attempt", "\nasync fn finish_submit_attempt"),
        "postgres idempotency adapter",
        "finish_submit_attempt",
        ["finish::finish_submit_attempt(self, attempt).await"],
    )
    begin_text = (STORE_SRC / "postgres_idempotency/begin.rs").read_text()
    for needle in ["SELECT pg_advisory_xact_lock($1)", 'status == "PROCEEDING"', "IdempotencyAction::InProgress", "IdempotencyAction::Proceed"]:
        if needle not in begin_text:
            fail(f"postgres idempotency begin path missing token: {needle}")
    idempotency_tests = (STORE_SRC / "postgres_tests/idempotency.rs").read_text()
    for test_name in ["same_request_replay_is_persisted", "fingerprint_mismatch_is_conflict"]:
        if test_name not in idempotency_tests:
            fail(f"postgres idempotency tests missing token: {test_name}")
    reservation_tests = (STORE_SRC / "postgres_tests/receipt_reservation.rs").read_text()
    for test_name in ["reservation_double_spend_is_prevented_concurrently", "remote_unknown_is_persisted_conservatively"]:
        if test_name not in reservation_tests:
            fail(f"postgres receipt/reservation tests missing token: {test_name}")


def validate_v07_source_landings() -> None:
    traits_text = (GATEWAY_SRC / "traits.rs").read_text()
    for trait_name in ["SignerProvider", "ClobGateway", "RemoteReconcileReader"]:
        if f"trait {trait_name}" not in traits_text:
            fail(f"gateway traits missing token: pub trait {trait_name}")
    try:
        signer_provider_methods = rust_trait_method_signatures(traits_text, "SignerProvider")
        clob_gateway_methods = rust_trait_method_signatures(traits_text, "ClobGateway")
        reconcile_reader_methods = rust_trait_method_signatures(traits_text, "RemoteReconcileReader")
    except SystemExit as exc:
        fail(f"gateway traits malformed: {exc}")
    if signer_provider_methods != {"signer_for_account"}:
        fail(f"gateway traits missing token: pub trait SignerProvider")
    if not {"post_order", "cancel_order", "get_order", "get_open_orders"}.issubset(clob_gateway_methods):
        fail("gateway traits missing token: async fn post_order(&self, order: &SignedOrderEnvelope)")
    if reconcile_reader_methods != {"read_remote_order_observations"}:
        fail("gateway traits missing token: pub trait RemoteReconcileReader")
    signer_text = (GATEWAY_SRC / "signer.rs").read_text()
    for needle in [
        "pub struct DeterministicTestSignerProvider",
        "pub struct DeterministicTestSigner",
        "pub struct SignerProviderConfig",
        "allow_local_private_key_material: false",
        "require_remote_signer_in_production: true",
    ]:
        if needle not in signer_text:
            fail(f"gateway signer provider missing token: {needle}")
    signer_tests = (GATEWAY_SRC / "tests/signer.rs").read_text()
    for test_name in ["disabled_signer_provider_refuses_to_materialize_signer", "signer_provider_defaults_are_production_conservative"]:
        if test_name not in signer_tests:
            fail(f"gateway signer tests missing token: {test_name}")
    post_cancel_tests = (GATEWAY_SRC / "tests/post_cancel.rs").read_text()
    for test_name in ["deterministic_signer_provider_posts_reads_and_cancels", "fake_gateway_surfaces_remote_unknown_without_local_success"]:
        if test_name not in post_cancel_tests:
            fail(f"gateway post/cancel tests missing token: {test_name}")
    require_file_tokens(
        API_E2E_TEST.parent / "http_and_fake_e2e/scaffold.rs",
        "HTTP scaffold E2E",
        ["full_scaffold_path_compile_submit_cancel_and_reconcile", "compile_blocked_plan", "verify_submit_and_sign_only", "verify_non_live_admin_paths", "verify_public_queries"],
    )
    require_file_tokens(
        API_E2E_TEST.parent / "http_and_fake_e2e/scaffold/admin_paths.rs",
        "HTTP scaffold admin paths",
        ["/v1/admin/cancel-order", "/v1/admin/reconcile", "/v1/admin/reconcile-order-local", "StatusCode::ACCEPTED", "StatusCode::NOT_FOUND"],
    )
    require_file_tokens(
        API_E2E_TEST.parent / "http_and_fake_e2e/scaffold/submit_sign_only.rs",
        "HTTP scaffold submit/sign-only",
        ["/v1/submissions", "/v1/sign-only/standard-constructions", "/v1/sign-only/lifecycle-events", '"mode": "BLOCKED_DRY_RUN"', "StatusCode::BAD_REQUEST"],
    )
    require_file_tokens(
        SDK_SPIKE_RS,
        "official SDK spike",
        ["OFFICIAL_SDK_REPOSITORY", "PINNED_OFFICIAL_SDK_VERSION", "LIVE_SUBMIT_FEATURE_NAME", "allow_live_submit: false", "require_explicit_runtime_kill_switch_open: true", "sdk_client_type_marker"],
    )


def validate_v08_dependency_and_sdk_policy() -> None:
    require_file_tokens(
        ROOT_CARGO_TOML,
        "root Cargo baseline",
        [
            'edition = "2024"',
            'rust-version = "1.88"',
            f'version = "{(ROOT / "VERSION").read_text().strip()}"',
            'tokio = { version = "1.52.3"',
            'serde = { version = "1.0.228"',
        ],
    )
    require_file_tokens(
        EXECUTOR / "rust-toolchain.toml",
        "executor rust toolchain",
        ['channel = "1.88.0"', 'components = ["rustfmt", "clippy"]', 'profile = "minimal"'],
    )
    require_file_tokens(
        SDK_SPIKE_TOML,
        "official SDK spike Cargo",
        ['name = "pmx-official-sdk-spike"', 'rust-version = "1.88"', 'sdk-typecheck = ["dep:polymarket_client_sdk_v2"]', 'live-submit = ["sdk-typecheck"]', 'polymarket_client_sdk_v2 = { version = "=0.6.0-canary.1"'],
    )
    require_file_tokens(
        SDK_SPIKE_RS,
        "official SDK spike",
        ['PINNED_OFFICIAL_SDK_VERSION: &str = "=0.6.0-canary.1"', 'read_only_ok_smoke', 'default_read_only_client', 'polymarket_client_sdk_v2::clob::Client::new', 'client.server_time()'],
    )
    sdk_text = SDK_SPIKE_RS.read_text()
    sdk_toml = SDK_SPIKE_TOML.read_text()
    if 'use std::time::Duration;' in sdk_text and '#[cfg(feature = "sdk-typecheck")]\n    use std::time::Duration;' not in sdk_text:
        fail("Duration import must be cfg-gated to avoid no-feature warning")
    if 'polymarket_client_sdk_v2 = { version = "=0.6.0-canary.1"' not in sdk_toml:
        fail("official SDK dependency must remain explicitly pinned in spike")
    for doc in [
        ROOT / "DEPENDENCY_POLICY.md",
        EXECUTOR / "docs/SDK_FIRST_ADAPTER_PLAN.md",
        EXECUTOR / "rust-toolchain.toml",
        ROOT / ".github/dependabot.yml",
        ROOT / ".github/workflows/ci.yml",
    ]:
        if not doc.exists():
            fail(f"v0.11 missing dependency/CI artifact: {doc.relative_to(ROOT)}")


def validate_v09_official_adapter_boundary() -> None:
    if not SDK_ADAPTER_RS.exists():
        fail("missing official SDK adapter boundary crate source")
    if not SDK_ADAPTER_TOML.exists():
        fail("missing official SDK adapter boundary Cargo.toml")
    adapter_toml = cargo_toml(SDK_ADAPTER_TOML)
    adapter_text = SDK_ADAPTER_RS.read_text()
    for needle in [
        "Official Polymarket SDK adapter boundary",
        "sign-only dry-runs require explicit opt-in",
        "live submit requires the explicit `live-submit` feature",
        "pub use gates::*",
    ]:
        if needle not in adapter_text:
            fail(f"official SDK adapter root missing token: {needle}")
    adapter_modules = rust_module_names(adapter_text, "mod")
    for module_name in ["gates", "lifecycle", "liveness", "mapping", "model", "redaction", "sdk_runtime", "standard_sign_only"]:
        if module_name not in adapter_modules:
            fail(f"official SDK adapter root missing token: mod {module_name};")
    if "gates" not in rust_pub_use_targets(adapter_text):
        fail("official SDK adapter root missing token: pub use gates::*")
    feature_names = set(adapter_toml.get("features", {}).keys())
    for feature_name in ["sdk-typecheck", "authenticated-smoke", "sign-only-dry-run", "live-submit"]:
        if feature_name not in feature_names:
            fail(f"official adapter must expose feature: {feature_name}")
    sdk_dep = adapter_toml.get("dependencies", {}).get("polymarket_client_sdk_v2", {})
    if sdk_dep.get("version") != "=0.6.0-canary.1":
        fail("official adapter must pin the official SDK canary explicitly")
    bins = {entry.get("name"): entry for entry in adapter_toml.get("bin", [])}
    if bins.get("pmx-real-funds-canary", {}).get("required-features") != ["live-submit"]:
        fail("official adapter must gate pmx-real-funds-canary behind live-submit")
    config_text = (SDK_ADAPTER_SRC / "model/config.rs").read_text()
    sdk_error_text = (SDK_ADAPTER_SRC / "model/sdk_error.rs").read_text()
    sign_only_model_text = (SDK_ADAPTER_SRC / "model/sign_only.rs").read_text()
    try:
        adapter_config_fields = rust_struct_field_names(
            config_text, "OfficialSdkAdapterConfig"
        )
        standard_profile_fields = rust_struct_field_names(
            config_text, "OfficialSdkStandardSignOnlyProfile"
        )
        sdk_error_variants = rust_enum_variant_names(
            sdk_error_text, "OfficialSdkErrorCategory"
        )
        normalized_error_fields = rust_struct_field_names(
            sdk_error_text, "OfficialSdkNormalizedError"
        )
        dry_run_receipt_fields = rust_struct_field_names(
            sign_only_model_text, "SignOnlyDryRunReceipt"
        )
    except SystemExit as exc:
        fail(f"official SDK adapter models malformed: {exc}")
    if adapter_config_fields != {
        "clob_host",
        "allow_read_only_smoke",
        "allow_authenticated_non_trading_smoke",
        "allow_sign_only_dry_run",
        "allow_live_submit",
        "allow_real_funds_canary",
        "require_kill_switch_open_for_live_submit",
        "require_repository_reservation_for_live_submit",
        "require_reconcile_worker_for_live_submit",
    }:
        fail("official SDK adapter config missing OfficialSdkAdapterConfig fields")
    if not {
        "clob_host",
        "collateral_symbol",
        "signing_protocol",
        "uses_deposit_wallet_order_path",
        "supports_builder_attribution",
        "supports_fee_metadata",
        "exposes_raw_signed_order",
        "may_post_order",
        "may_cancel_order",
    }.issubset(standard_profile_fields):
        fail(
            "official SDK adapter config missing OfficialSdkStandardSignOnlyProfile fields"
        )
    if sdk_error_variants != {
        "RemoteRejected",
        "RemoteUnknown",
        "AuthenticationFailed",
        "ValidationFailed",
        "Geoblocked",
        "WebSocketFailed",
        "Internal",
    }:
        fail("official SDK error model missing OfficialSdkErrorCategory variants")
    if normalized_error_fields != {
        "category",
        "retryable",
        "message",
        "http_status",
        "geoblock_country",
        "geoblock_region",
    }:
        fail("official SDK error model missing OfficialSdkNormalizedError fields")
    if dry_run_receipt_fields != {
        "account_id",
        "execution_id",
        "plan_hash",
        "signed_order_ref",
        "posted",
    }:
        fail("official SDK sign-only model missing SignOnlyDryRunReceipt fields")
    require_tokens(
        config_text,
        "official SDK adapter config",
        [
            "allow_sign_only_dry_run: false",
            "allow_live_submit: false",
            "allow_real_funds_canary: false",
            "may_post_order: false",
            "may_cancel_order: false",
            "exposes_raw_signed_order: false",
        ],
    )
    smoke_text = (SDK_ADAPTER_SRC / "gates/smoke.rs").read_text()
    smoke_fns = rust_fn_names(smoke_text)
    if not {"validate_read_only_smoke_environment", "validate_authenticated_non_trading_smoke", "validate_sign_only_dry_run"}.issubset(
        smoke_fns
    ):
        fail("official SDK smoke gates missing validation functions")
    for env_name in ["ENV_RUN_AUTHENTICATED_SMOKE", "ENV_ALLOW_SIGN_ONLY_DRY_RUN", "ENV_ALLOW_LIVE_SUBMIT"]:
        if env_name not in smoke_text:
            fail(f"official SDK smoke gates missing token: {env_name}")
    sign_only_text = (SDK_ADAPTER_SRC / "sdk_runtime/sign_only/dry_run.rs").read_text()
    sign_only_fns = rust_async_fn_names(sign_only_text)
    if "run_sign_only_dry_run" not in sign_only_fns:
        fail("official SDK sign-only dry run missing async entrypoint")
    require_tokens(
        sign_only_text,
        "official SDK sign-only dry run",
        [
            "validate_sign_only_dry_run(config, &credentials)?",
            "signed_order_ref = format!(",
            "posted: false",
        ],
    )
    feature_tests = (SDK_ADAPTER_SRC / "tests/feature_gated.rs").read_text()
    feature_test_fns = rust_fn_names(feature_tests) | rust_async_fn_names(feature_tests)
    if not {
        "env_helpers_trim_values_and_accept_case_insensitive_true",
        "authenticated_non_trading_smoke_executes_when_enabled",
        "sign_only_dry_run_executes_when_enabled",
    }.issubset(feature_test_fns):
        fail("official SDK feature-gated tests missing required test functions")
    require_tokens(
        feature_tests,
        "official SDK feature-gated tests",
        [
            "sdk_error_normalization_covers_validation",
            "sdk_error_normalization_covers_status_codes",
            "gateway_error_conversion_preserves_remote_unknown",
            "assert!(!receipt.posted);",
        ],
    )
    live_canary = SDK_ADAPTER_SRC / "sdk_runtime/live_canary.rs"
    gateway_bridge = SDK_ADAPTER_SRC / "sdk_runtime/gateway.rs"
    adapter_boundary_text = source_tree_text_without_paths(SDK_ADAPTER_SRC, [live_canary, gateway_bridge])
    validate_absent_tokens(
        adapter_boundary_text,
        "official adapter boundary",
        ["post_order(", "post_orders("],
    )
    if gateway_bridge.exists():
        gateway_text = gateway_bridge.read_text()
        for needle in [
            "allow_live_submit",
            "OfficialSdkGateway",
            "official-sdk-signed:",
            "signed_payload_ref",
            "SignedOrderEnvelope",
            "redact_sensitive_text",
        ]:
            if needle not in gateway_text:
                fail(f"official SDK gateway bridge missing boundary token: {needle}")
    if 'allow_live_submit: false' not in rust_source_text(SDK_ADAPTER_SRC):
        fail("official adapter default must keep live submit disabled")
    for doc in [
        EXECUTOR / "docs/AUTHENTICATED_NON_TRADING_SMOKE.md",
        EXECUTOR / "docs/SIGN_ONLY_DRY_RUN.md",
        EXECUTOR / "docs/OFFICIAL_SDK_ADAPTER_BOUNDARY.md",
        ROOT / "NO_LOCAL_ACTIONS_REMAINING.md",
        EXECUTOR / "validation/run_current_gates.sh",
    ]:
        if not doc.exists():
            fail(f"v0.11 missing continuation artifact: {doc.relative_to(ROOT)}")


def validate_v12_service_layer(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    api_text = rust_source_text(API_SRC)
    if not SERVICE_RS.exists():
        fail("missing pmx-service crate source")
    if not SERVICE_TOML.exists():
        fail("missing pmx-service Cargo.toml")
    backend_text = (API_SRC / "backend.rs").read_text()
    expected_openapi_refs = {
        ("/v1/plans/compile", "post", "request"): "#/components/schemas/CompilePlanRequest",
        ("/v1/plans/compile", "post", "200"): "#/components/schemas/ExecutionPlanSummary",
        ("/v1/submissions", "post", "request"): "#/components/schemas/SubmitRequest",
        ("/v1/submissions", "post", "202"): "#/components/schemas/SubmitReceipt",
        ("/v1/admin/cancel-order", "post", "request"): "#/components/schemas/CancelOrderRequest",
        ("/v1/admin/cancel-order", "post", "202"): "#/components/schemas/CancelReceipt",
        ("/v1/admin/reconcile", "post", "request"): "#/components/schemas/ReconcileRequest",
        ("/v1/admin/reconcile", "post", "202"): "#/components/schemas/ReconcileReport",
    }
    for (path, method, kind), expected_ref in expected_openapi_refs.items():
        actual_ref = operation_request_ref(spec, path, method) if kind == "request" else operation_response_ref(spec, path, method, kind)
        if actual_ref != expected_ref:
            fail(f"OpenAPI {method.upper()} {path} {kind} must reference {expected_ref}, got {actual_ref}")
    require_file_tokens(
        SERVICE_TOML,
        "pmx-service Cargo",
        ['name = "pmx-service"'],
    )
    service_root_text = SERVICE_RS.read_text()
    plan_route_text = (API_SRC / "routes/flow/plan.rs").read_text()
    module_names = rust_module_names(service_root_text)
    if not {"binding", "plan_flow", "submit"}.issubset(module_names):
        fail("pmx-service crate root missing required modules")
    pub_use_targets = rust_pub_use_targets(service_root_text)
    if not {"binding", "submit"}.issubset(pub_use_targets):
        fail("pmx-service crate root missing required pub use re-exports")
    backend_variants = rust_enum_variant_names(backend_text, "ServiceBackend")
    if backend_variants != {"InMemory", "Postgres"}:
        fail("pmx-api backend structure missing ServiceBackend variants")
    app_state_fields = rust_struct_field_names(backend_text, "AppState")
    app_state_fns = rust_fn_names(backend_text)
    if not {"in_memory", "in_memory_with_store", "postgres"}.issubset(app_state_fns):
        fail("pmx-api backend structure missing AppState constructors")
    if app_state_fields != {"service"}:
        fail("pmx-api backend structure missing AppState fields")
    in_memory_body = rust_impl_method_body(backend_text, "AppState", "in_memory")
    in_memory_with_store_body = rust_impl_method_body(
        backend_text, "AppState", "in_memory_with_store"
    )
    postgres_body = rust_impl_method_body(backend_text, "AppState", "postgres")
    hash_inputs_text = (SERVICE_SRC / "binding/hash_inputs.rs").read_text()
    try:
        plan_hash_fields = rust_struct_field_names(hash_inputs_text, "PlanHashInput")
        approval_hash_fields = rust_struct_field_names(hash_inputs_text, "ApprovalHashInput")
        approval_hash_from_body = rust_impl_trait_method_body(
            hash_inputs_text, "From<&'a ApprovalReceipt>", "ApprovalHashInput<'a>", "from"
        )
        plan_hash_from_body = rust_impl_trait_method_body(
            hash_inputs_text, "From<&'a ExecutionPlanSummary>", "PlanHashInput<'a>", "from"
        )
    except SystemExit as exc:
        fail(f"service binding hash inputs malformed: {exc}")
    if not {
        "approval_id",
        "approval_hash",
        "executor_version",
        "contract_version",
    }.issubset(plan_hash_fields):
        fail("service binding hash inputs missing required PlanHashInput fields")
    if "bound_snapshot_hash" not in approval_hash_fields:
        fail("service binding hash inputs missing required ApprovalHashInput fields")
    require_tokens(
        approval_hash_from_body,
        "service binding hash inputs",
        [
            "bound_snapshot_hash: &approval.bound_snapshot_hash",
            "bound_decision_hash: &approval.bound_decision_hash",
            "bound_plan_hash: &approval.bound_plan_hash",
        ],
    )
    require_tokens(
        plan_hash_from_body,
        "service binding hash inputs",
        [
            "approval_hash: &plan.approval_hash",
            "executor_version: &plan.executor_version",
            "contract_version: &plan.contract_version",
        ],
    )
    if not {"compile_plan", "submit_plan"}.issubset(rust_async_fn_names(plan_route_text)):
        fail("API plan flow routes missing compile/submit handlers")
    require_file_tokens(
        SERVICE_SRC / "submit/blocked.rs",
        "blocked submit path",
        ['"SUBMIT_BLOCKED_BEFORE_REMOTE"', '"no_remote_side_effect": true', '"reservation_written": false', "finish_submit_attempt(pmx_store::FinishSubmitAttempt"],
    )
    blocked_submit = (SERVICE_SRC / "submit/blocked.rs").read_text()
    validate_absent_tokens(blocked_submit, "blocked submit path", ["save_order_reservation"])
    require_file_tokens(
        SERVICE_SRC / "service_tests/flow.rs",
        "service flow tests",
        ["service_flow_persists_and_blocks_submit", "service_id_bound_flow_persists_and_blocks_submit", "service_rejects_object_graph_mismatch", "service_rejects_tampered_approval_hash"],
    )
    bootstrap_text = (API_SRC / "routes/bootstrap.rs").read_text()
    route_paths = set(re.findall(r'\.route\(\s*"([^"]+)"', bootstrap_text))
    if not {
        "/v1/plans/compile",
        "/v1/submissions",
        "/v1/admin/cancel-order",
        "/v1/admin/reconcile",
    }.issubset(route_paths):
        fail("API bootstrap routes missing required paths")
    require_tokens(
        bootstrap_text,
        "API bootstrap routes",
        ["pub async fn try_postgres_app(", "AppState::postgres(store)"],
    )
    require_tokens(
        in_memory_body,
        "pmx-api backend structure",
        ["ServiceBackend::InMemory", "ExecutorService::new(InMemoryStore::default())"],
    )
    require_tokens(
        in_memory_with_store_body,
        "pmx-api backend structure",
        ["ServiceBackend::InMemory", "ExecutorService::new(store)"],
    )
    require_tokens(
        postgres_body,
        "pmx-api backend structure",
        [
            "StoreBackedRuntimeStateProvider::new(store.clone())",
            "ExecutorService::with_runtime_provider(",
            "ServiceBackend::Postgres",
        ],
    )
    validate_absent_tokens(api_text, "pmx-api", ["pub fn fake_snapshot"])
    if not (EXECUTOR / "validation/run_current_gates.sh").exists():
        fail("missing current gate runner")
    if not API_POSTGRES_E2E_TEST.exists():
        fail("missing HTTP PostgreSQL E2E test source")
    pg_test_text = rust_file_with_modules_text(API_POSTGRES_E2E_TEST)
    for needle in [
        'http_postgres_backed_e2e_smoke',
        'http_postgres_rejects_cross_object_graph_and_bad_plan_hash',
        'PMX_TEST_DATABASE_URL',
        'try_postgres_app',
        '"database"], "postgres"',
        'StatusCode::ACCEPTED',
        'StatusCode::OK',
    ]:
        if needle not in pg_test_text:
            fail(f"PostgreSQL API E2E missing token: {needle}")


def validate_v15_admin_audit_and_runtime_provider(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    pg_test_text = rust_file_with_modules_text(API_POSTGRES_E2E_TEST)
    audit_operation = openapi_operation(spec, "/v1/admin/audit-events", "get")
    if operation_response_array_item_ref(spec, "/v1/admin/audit-events", "get", "200") != "#/components/schemas/AdminAuditEvent":
        fail("v0.15 admin audit response must be an array of AdminAuditEvent")
    if operation_request_ref(spec, "/v1/admin/kill-switch", "post") != "#/components/schemas/KillSwitchRequest":
        fail("v0.15 kill-switch request must reference KillSwitchRequest")
    if operation_response_ref(spec, "/v1/admin/kill-switch", "post", "202") != "#/components/schemas/KillSwitchReceipt":
        fail("v0.15 kill-switch response must reference KillSwitchReceipt")
    audit_params = operation_parameter_names(audit_operation)
    for required_param in {"before_audit_id", "operation", "principal_subject", "result", "correlation_id"}:
        if required_param not in audit_params:
            fail(f"v0.15 admin audit query must expose {required_param}")
    audit_model_text = (STORE_SRC / "model/audit.rs").read_text()
    memory_audit_text = (STORE_SRC / "memory/audit.rs").read_text()
    postgres_audit_text = (STORE_SRC / "postgres_audit/admin.rs").read_text()
    service_audit_text = (SERVICE_SRC / "service/audit.rs").read_text()
    api_backend_audit_text = (API_SRC / "backend/audit.rs").read_text()
    api_route_audit_text = (API_SRC / "routes/admin/audit.rs").read_text()
    api_support_audit_text = (API_SRC / "support/audit.rs").read_text()
    try:
        audit_event_fields = rust_struct_field_names(audit_model_text, "AdminAuditEvent")
        audit_query_fields = rust_struct_field_names(audit_model_text, "AdminAuditQuery")
        audit_store_methods = rust_trait_method_signatures(audit_model_text, "AdminAuditStore")
        memory_audit_impl_methods = rust_impl_trait_method_names(
            memory_audit_text, "AdminAuditStore", "InMemoryStore"
        )
        memory_audit_record_body = rust_impl_trait_method_body(
            memory_audit_text, "AdminAuditStore", "InMemoryStore", "record_admin_audit_event"
        )
        memory_audit_list_body = rust_impl_trait_method_body(
            memory_audit_text, "AdminAuditStore", "InMemoryStore", "list_admin_audit_events"
        )
        postgres_audit_impl_methods = rust_impl_trait_method_names(
            postgres_audit_text, "AdminAuditStore", "PostgresStore"
        )
        postgres_audit_record_body = rust_impl_trait_method_body(
            postgres_audit_text, "AdminAuditStore", "PostgresStore", "record_admin_audit_event"
        )
        postgres_audit_list_body = rust_impl_trait_method_body(
            postgres_audit_text, "AdminAuditStore", "PostgresStore", "list_admin_audit_events"
        )
    except SystemExit as exc:
        fail(f"store admin audit model malformed: {exc}")
    if not {
        "audit_id",
        "principal_subject",
        "operation",
        "request_fingerprint",
        "correlation_id",
        "result",
        "created_at",
    }.issubset(audit_event_fields):
        fail("store admin audit model missing AdminAuditEvent fields")
    if not {
        "limit",
        "before_audit_id",
        "operation",
        "principal_subject",
        "result",
        "correlation_id",
    }.issubset(audit_query_fields):
        fail("store admin audit model missing AdminAuditQuery fields")
    if audit_store_methods != {"record_admin_audit_event", "list_admin_audit_events"}:
        fail("store admin audit model missing AdminAuditStore methods")
    if "record_admin_audit_event" not in rust_async_fn_names(service_audit_text):
        fail("service admin audit bridge missing record_admin_audit_event")
    if "list_admin_audit_events" not in rust_async_fn_names(service_audit_text):
        fail("service admin audit bridge missing list_admin_audit_events")
    if "record_admin_audit_event" not in rust_async_fn_names(api_backend_audit_text):
        fail("API admin audit backend missing record_admin_audit_event")
    if "list_admin_audit_events" not in rust_async_fn_names(api_backend_audit_text):
        fail("API admin audit backend missing list_admin_audit_events")
    if "list_admin_audit_events" not in rust_async_fn_names(api_route_audit_text):
        fail("API admin audit routes missing list_admin_audit_events")
    if "record_admin_audit" not in rust_async_fn_names(api_support_audit_text):
        fail("API admin audit support missing record_admin_audit")
    if memory_audit_impl_methods != {"record_admin_audit_event", "list_admin_audit_events"}:
        fail("in-memory admin audit store missing AdminAuditStore impl methods")
    if postgres_audit_impl_methods != {"record_admin_audit_event", "list_admin_audit_events"}:
        fail("postgres admin audit store missing AdminAuditStore impl methods")
    require_tokens(
        memory_audit_record_body,
        "in-memory admin audit store",
        ["sanitize_admin_audit_event", "state.admin_audit.push(stored)", "stored.created_at = Some(Utc::now())"],
    )
    require_tokens(
        memory_audit_list_body,
        "in-memory admin audit store",
        ["correlation_id", "event.correlation_id.as_ref() == Some(correlation_id)", "query.bounded_limit()"],
    )
    require_tokens(
        postgres_audit_record_body,
        "postgres admin audit store",
        ["INSERT INTO admin_audit_events", "&event.correlation_id", ".map_err(map_db_error)?"],
    )
    require_tokens(
        postgres_audit_list_body,
        "postgres admin audit store",
        ["FROM admin_audit_events", "correlation_id = $6", "query.bounded_limit()"],
    )
    require_tokens(
        service_audit_text,
        "service admin audit bridge",
        ["AdminAuditStore", "self.store.record_admin_audit_event(&event).await?", "pub async fn list_admin_audit_events"],
    )
    require_tokens(
        api_backend_audit_text,
        "API admin audit backend",
        ["Self::InMemory(service) => service.record_admin_audit_event(event).await", "Self::Postgres(service) => service.list_admin_audit_events(query).await"],
    )
    require_tokens(
        api_route_audit_text,
        "API admin audit routes",
        ["AdminAuditQuery", "correlation_id: query.correlation_id", "StatusCode::OK"],
    )
    require_tokens(
        api_support_audit_text,
        "API admin audit support",
        ["operation: &'static str", "record_admin_audit_event(AdminAuditEvent", "principal_subject: principal.subject.clone()"],
    )
    require_file_tokens(
        API_SRC / "routes/health.rs",
        "API health route",
        ['"service_layer": "pmx_service_server_authoritative_id_bound_admin_audit"'],
    )
    for needle in ["http_postgres_admin_routes_record_audit_events", "admin_audit_events", "KillSwitch", "CancelOrder"]:
        if needle not in pg_test_text:
            fail(f"PostgreSQL API audit E2E missing token: {needle}")
    if not (EXECUTOR / "validation/run_current_gates.sh").exists():
        fail("missing current gate runner")


def validate_v16_postgres_runtime_provider(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    backend_text = (API_SRC / "backend.rs").read_text()
    health_text = (API_SRC / "routes/health.rs").read_text()
    pg_test_text = rust_file_with_modules_text(API_POSTGRES_E2E_TEST)
    spike_text = SDK_SPIKE_RS.read_text()
    memory_runtime_state_text = (STORE_SRC / "memory/runtime/state.rs").read_text()
    memory_runtime_support_text = (STORE_SRC / "memory/runtime/support.rs").read_text()
    postgres_runtime_text = (STORE_SRC / "postgres_runtime.rs").read_text()
    postgres_worker_status_text = (STORE_SRC / "postgres_worker/status.rs").read_text()
    store_backed_runtime_text = (SERVICE_SRC / "runtime_state/store_backed.rs").read_text()
    runtime_worker_ref = operation_response_ref(spec, "/v1/runtime/workers", "get", "200")
    if runtime_worker_ref != "#/components/schemas/RuntimeWorkerStatusReport":
        fail("v0.16 runtime workers response must reference RuntimeWorkerStatusReport")
    runtime_model_text = (STORE_SRC / "model/runtime.rs").read_text()
    try:
        runtime_state_query_fields = rust_struct_field_names(runtime_model_text, "RuntimeStateQuery")
        runtime_worker_status_report_fields = rust_struct_field_names(
            runtime_model_text, "RuntimeWorkerStatusReport"
        )
        runtime_state_methods = rust_trait_method_signatures(runtime_model_text, "RuntimeStateStore")
        runtime_worker_methods = rust_trait_method_signatures(
            runtime_model_text, "RuntimeWorkerStatusStore"
        )
        runtime_control_methods = rust_trait_method_signatures(
            runtime_model_text, "RuntimeControlStore"
        )
        memory_runtime_state_impl_methods = rust_impl_trait_method_names(
            memory_runtime_state_text, "RuntimeStateStore", "InMemoryStore"
        )
        memory_runtime_state_body = rust_impl_trait_method_body(
            memory_runtime_state_text, "RuntimeStateStore", "InMemoryStore", "load_runtime_state"
        )
        postgres_runtime_state_impl_methods = rust_impl_trait_method_names(
            postgres_runtime_text, "RuntimeStateStore", "PostgresStore"
        )
        postgres_runtime_state_body = rust_impl_trait_method_body(
            postgres_runtime_text, "RuntimeStateStore", "PostgresStore", "load_runtime_state"
        )
        postgres_runtime_control_impl_methods = rust_impl_trait_method_names(
            postgres_runtime_text, "RuntimeControlStore", "PostgresStore"
        )
        postgres_runtime_account_kill_switch_body = rust_impl_trait_method_body(
            postgres_runtime_text,
            "RuntimeControlStore",
            "PostgresStore",
            "set_account_kill_switch",
        )
        postgres_runtime_global_kill_switch_body = rust_impl_trait_method_body(
            postgres_runtime_text,
            "RuntimeControlStore",
            "PostgresStore",
            "set_global_kill_switch",
        )
        postgres_runtime_worker_impl_methods = rust_impl_trait_method_names(
            postgres_worker_status_text, "RuntimeWorkerStatusStore", "PostgresStore"
        )
        postgres_runtime_worker_status_body = rust_impl_trait_method_body(
            postgres_worker_status_text,
            "RuntimeWorkerStatusStore",
            "PostgresStore",
            "list_runtime_worker_status",
        )
        memory_runtime_support_methods = rust_inherent_impl_method_names(
            memory_runtime_support_text, "InMemoryStore"
        )
        memory_runtime_support_set_body = rust_impl_method_body(
            memory_runtime_support_text, "InMemoryStore", "set_runtime_state_for_test"
        )
        memory_runtime_support_observations_body = rust_impl_method_body(
            memory_runtime_support_text, "InMemoryStore", "observations_for_account"
        )
        store_backed_runtime_fields = rust_struct_field_names(
            store_backed_runtime_text, "StoreBackedRuntimeStateProvider"
        )
        store_backed_runtime_methods = rust_inherent_impl_method_names(
            store_backed_runtime_text, "StoreBackedRuntimeStateProvider<S>"
        )
        store_backed_runtime_capture_body = rust_impl_trait_method_body(
            store_backed_runtime_text,
            "RuntimeStateProvider",
            "StoreBackedRuntimeStateProvider<S>",
            "capture_runtime_state",
        )
    except SystemExit as exc:
        fail(f"store runtime model malformed: {exc}")
    if not {"account_id", "condition_id", "collateral_profile_id", "required_capabilities"}.issubset(
        runtime_state_query_fields
    ):
        fail("store runtime model missing RuntimeStateQuery fields")
    if runtime_worker_status_report_fields != {"heartbeats", "observations"}:
        fail("store runtime model missing RuntimeWorkerStatusReport fields")
    if runtime_state_methods != {"load_runtime_state"}:
        fail("store runtime model missing RuntimeStateStore methods")
    if runtime_worker_methods != {"list_runtime_worker_status"}:
        fail("store runtime model missing RuntimeWorkerStatusStore methods")
    if runtime_control_methods != {"set_account_kill_switch", "set_global_kill_switch"}:
        fail("store runtime model missing RuntimeControlStore methods")
    if memory_runtime_support_methods != {"set_runtime_state_for_test", "observations_for_account"}:
        fail("in-memory runtime support missing required InMemoryStore helper methods")
    if store_backed_runtime_fields != {"store", "required_capabilities"}:
        fail("service store-backed runtime provider missing StoreBackedRuntimeStateProvider fields")
    if not {"new", "with_required_capabilities", "load_canary_runtime_truth"}.issubset(
        store_backed_runtime_methods
    ):
        fail("service store-backed runtime provider missing constructor/canary helper methods")
    if "capture_runtime_state" not in rust_async_fn_names(store_backed_runtime_text):
        fail("service store-backed runtime provider missing runtime capture/canary truth methods")
    if memory_runtime_state_impl_methods != {"load_runtime_state"}:
        fail("in-memory runtime state store missing RuntimeStateStore impl methods")
    require_tokens(
        memory_runtime_state_body,
        "in-memory runtime state store",
        ["query.state_scope_key()", "worker_status_from_heartbeats", "global_kill_switch", "apply_runtime_worker_observations"],
    )
    require_tokens(
        memory_runtime_support_set_body,
        "in-memory runtime support",
        ["query.state_scope_key()", ".runtime_states", ".insert("],
    )
    require_tokens(
        memory_runtime_support_observations_body,
        "in-memory runtime support",
        ["runtime_observation_is_fresh(observation)", "observation.account_id == account_id", ".runtime_worker_observations"],
    )
    if postgres_runtime_state_impl_methods != {"load_runtime_state"}:
        fail("postgres runtime state store missing RuntimeStateStore impl methods")
    if postgres_runtime_control_impl_methods != {"set_account_kill_switch", "set_global_kill_switch"}:
        fail("postgres runtime state store missing RuntimeControlStore impl methods")
    require_tokens(
        postgres_runtime_state_body,
        "postgres runtime state store",
        ["IsolationLevel::RepeatableRead", "account_collateral::load_account_state", "worker_rows::load_worker_rows", "apply_runtime_worker_observations"],
    )
    require_tokens(
        postgres_runtime_account_kill_switch_body,
        "postgres runtime state store",
        ["self.client().await?", "account_collateral::set_account_kill_switch(&client, account_id, enabled, reason).await"],
    )
    require_tokens(
        postgres_runtime_global_kill_switch_body,
        "postgres runtime state store",
        ["self.client().await?", "account_collateral::set_global_kill_switch(&client, enabled, reason).await"],
    )
    if postgres_runtime_worker_impl_methods != {"list_runtime_worker_status"}:
        fail("postgres runtime worker status store missing RuntimeWorkerStatusStore impl methods")
    require_tokens(
        postgres_runtime_worker_status_body,
        "postgres runtime worker status store",
        ["FROM worker_health", "FROM runtime_worker_observations", "RuntimeWorkerStatusReport"],
    )
    require_tokens(
        store_backed_runtime_capture_body,
        "service store-backed runtime provider",
        ["load_runtime_state(&query)", "fail_closed_runtime_state(query.required_capabilities)"],
    )
    require_file_tokens(
        SERVICE_SRC / "service_tests/flow.rs",
        "service runtime provider flow tests",
        ["static_runtime_provider_can_reach_ready_plan_but_submit_still_blocks", "assert_eq!(plan.status, PlanStatus::Ready);", "SubmitMode::BlockedDryRun", "SubmitStatus::Blocked"],
    )
    require_file_tokens(
        API_SRC / "backend/runtime.rs",
        "API runtime backend",
        ["list_runtime_worker_status", "set_account_kill_switch", "set_global_kill_switch", ".store()", ".set_account_kill_switch(account_id, enabled, reason)", ".set_global_kill_switch(enabled, reason)"],
    )
    for needle in ["StoreBackedRuntimeStateProvider<PostgresStore>", "StoreBackedRuntimeStateProvider::new(store.clone())"]:
        if needle not in backend_text:
            fail(f"v0.16 API postgres runtime provider missing token: {needle}")
    for needle in ['Self::Postgres(_) => "postgres"', '"database": state.service.storage_mode()']:
        if needle not in (backend_text + "\n" + health_text):
            fail(f"v0.16 postgres health/backend structure missing token: {needle}")
    for needle in [
        "http_postgres_runtime_rows_can_reach_ready_plan_but_submit_still_blocks",
        "seed_allow_runtime",
        "REJECTED%",
        "execution_lifecycle_events",
    ]:
        if needle not in pg_test_text:
            fail(f"v0.16 PostgreSQL runtime/audit E2E missing token: {needle}")
    validate_absent_tokens(
        spike_text,
        "SDK spike read-only smoke",
        ["read-only smoke must not depend on signer credentials"],
    )
    for doc in [
        EXECUTOR / "docs/POSTGRES_RUNTIME_PROVIDER.md",
        EXECUTOR / "docs/ADMIN_AUDIT_FAILURE_PATHS.md",
        EXECUTOR / "validation/run_current_gates.sh",
        EXECUTOR / "docs/VALIDATION_PARTITIONING.md",
        EXECUTOR / "docs/RELEASE_HYGIENE_CLEAN_SNAPSHOT.md",
    ]:
        if not doc.exists():
            fail(f"v0.18 missing continuation artifact: {doc.relative_to(ROOT)}")


def validate_v19_redaction_and_live_guard(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    forbidden_public_tokens = {"SignedOrderEnvelope", "signed_payload", "private_key", "clob_secret", "post_order"}
    public_strings = set()
    stack = [spec]
    while stack:
        current = stack.pop()
        if isinstance(current, dict):
            for key, value in current.items():
                public_strings.add(str(key))
                stack.append(value)
        elif isinstance(current, list):
            stack.extend(current)
        elif isinstance(current, str):
            public_strings.add(current)
    for token in forbidden_public_tokens:
        if token in public_strings:
            fail(f"v0.19 public contract exposes forbidden live/signed term: {token}")
    redaction_text = (SDK_ADAPTER_SRC / "redaction.rs").read_text()
    redaction_fn_names = rust_fn_names(redaction_text)
    if not {
        "gateway_error_from_normalized_sdk_error",
        "redact_sensitive_text",
        "redact_normalized_error",
        "looks_like_hex_private_key",
        "redact_assignment_value",
    }.issubset(redaction_fn_names):
        fail("v0.19 adapter redaction missing required redaction function set")
    gateway_error_params, gateway_error_return = rust_fn_signature(
        redaction_text, "gateway_error_from_normalized_sdk_error"
    )
    if "&OfficialSdkNormalizedError" not in gateway_error_params or gateway_error_return != "GatewayError":
        fail("v0.19 adapter redaction must bind &OfficialSdkNormalizedError -> GatewayError")
    redact_sensitive_params, redact_sensitive_return = rust_fn_signature(
        redaction_text, "redact_sensitive_text"
    )
    if "&str" not in redact_sensitive_params or redact_sensitive_return != "String":
        fail("v0.19 adapter redaction must bind &str -> String")
    gateway_error_body = rust_fn_body(
        redaction_text, "gateway_error_from_normalized_sdk_error"
    )
    redact_sensitive_body = rust_fn_body(redaction_text, "redact_sensitive_text")
    constants_text = (SDK_ADAPTER_SRC / "model/constants.rs").read_text()
    if "REDACTED" not in rust_const_names(constants_text):
        fail("v0.19 adapter constants missing REDACTED constant")
    sdk_error_text = (SDK_ADAPTER_SRC / "model/sdk_error.rs").read_text()
    sdk_error_categories = rust_enum_variant_names(sdk_error_text, "OfficialSdkErrorCategory")
    if not {
        "RemoteRejected",
        "RemoteUnknown",
        "AuthenticationFailed",
        "ValidationFailed",
        "Geoblocked",
        "WebSocketFailed",
        "Internal",
    }.issubset(sdk_error_categories):
        fail("v0.19 SDK error categories missing required variants")
    normalized_error_fields = rust_struct_field_names(sdk_error_text, "OfficialSdkNormalizedError")
    if not {
        "category",
        "retryable",
        "message",
        "http_status",
        "geoblock_country",
        "geoblock_region",
    }.issubset(normalized_error_fields):
        fail("v0.19 normalized SDK error missing required fields")
    error_normalization_text = (SDK_ADAPTER_SRC / "liveness/error_normalization.rs").read_text()
    if "normalize_sdk_error" not in rust_fn_names(error_normalization_text):
        fail("v0.19 liveness error normalization missing normalize_sdk_error")
    normalization_params, normalization_return = rust_fn_signature(
        error_normalization_text, "normalize_sdk_error"
    )
    if "&SdkError" not in normalization_params or normalization_return != "OfficialSdkNormalizedError":
        fail("v0.19 liveness error normalization must bind &SdkError -> OfficialSdkNormalizedError")
    normalize_sdk_error_body = rust_fn_body(
        error_normalization_text, "normalize_sdk_error"
    )
    require_tokens(
        gateway_error_body,
        "v0.19 adapter redaction",
        [
            "OfficialSdkErrorCategory::AuthenticationFailed",
            "GatewayError::RemoteRejected(redact_sensitive_text(&normalized.message))",
            "GatewayError::RemoteUnknown(redact_sensitive_text(&normalized.message))",
        ],
    )
    require_tokens(
        redact_sensitive_body,
        "v0.19 adapter redaction",
        [
            "redact_known_env_values(input)",
            "looks_like_hex_private_key(token)",
            "\"0x[REDACTED]\".to_string()",
        ],
    )
    require_tokens(
        normalize_sdk_error_body,
        "v0.19 liveness error normalization",
        [
            "SdkErrorKind::Validation",
            "SdkErrorKind::Synchronization",
            "SdkErrorKind::Geoblock",
            "SdkErrorKind::WebSocket",
            "SdkErrorKind::Status",
            "SdkErrorKind::Internal",
            "OfficialSdkErrorCategory::RemoteRejected",
            "OfficialSdkErrorCategory::RemoteUnknown",
            "OfficialSdkErrorCategory::WebSocketFailed",
            "OfficialSdkErrorCategory::AuthenticationFailed",
            "OfficialSdkErrorCategory::Geoblocked",
            "OfficialSdkErrorCategory::ValidationFailed",
            "OfficialSdkErrorCategory::Internal",
        ],
    )
    liveness_error_tests = (SDK_ADAPTER_SRC / "tests/liveness_errors.rs").read_text()
    liveness_error_test_names = rust_fn_names(liveness_error_tests)
    if not {
        "gateway_error_conversion_redacts_sensitive_message",
        "normalized_error_redaction_covers_remote_unknown_messages",
        "redacts_private_key_like_hex_tokens",
        "redacts_named_secret_assignments",
    }.issubset(liveness_error_test_names):
        fail("v0.19 adapter redaction tests missing redaction coverage")
    if not LIVE_SUBMIT_GUARD.exists():
        fail("missing v0.19 live-submit static guard")
    guard_text = LIVE_SUBMIT_GUARD.read_text()
    for needle in ["post_order", "post_orders", "public OpenAPI", "live-submit static guard passed"]:
        if needle not in guard_text:
            fail(f"v0.19 live-submit static guard missing token: {needle}")
    if not (EXECUTOR / "validation/run_current_gates.sh").exists():
        fail("missing current gate runner")
    for doc in [EXECUTOR / "docs/SIGNED_PAYLOAD_REDACTION.md", EXECUTOR / "docs/LIVE_SUBMIT_STATIC_GUARD.md"]:
        if not doc.exists():
            fail(f"v0.19 missing safety doc: {doc.relative_to(ROOT)}")


def validate_v20_plan_storage_and_packaging(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    migration = (EXECUTOR / "migrations/0001_initial.sql").read_text()
    postgres = (EXECUTOR / "crates/pmx-store/src/postgres.rs").read_text()
    compile_request_ref = operation_request_ref(spec, "/v1/plans/compile", "post")
    compile_response_ref = operation_response_ref(spec, "/v1/plans/compile", "post", "200")
    if compile_request_ref != "#/components/schemas/CompilePlanRequest":
        fail("v0.20 compile plan request must reference CompilePlanRequest")
    if compile_response_ref != "#/components/schemas/ExecutionPlanSummary":
        fail("v0.20 compile plan response must reference ExecutionPlanSummary")
    create_tables = set(re.findall(r"CREATE TABLE IF NOT EXISTS ([a-z_]+)", migration))
    if "execution_plans" not in create_tables:
        fail("v0.20 migration must create canonical execution_plans table")
    if "DROP TABLE IF EXISTS plan_summaries" not in migration:
        fail("v0.20 migration plan storage missing token: DROP TABLE IF EXISTS plan_summaries")
    if "execution_plans.summary_json as canonical plan summary storage" not in migration:
        fail("v0.20 migration plan storage missing canonical summary_json note")
    execution_plans_body = re.search(r"(?s)CREATE TABLE IF NOT EXISTS execution_plans \((.*?)\);", migration)
    if not execution_plans_body:
        fail("v0.20 migration missing execution_plans table body")
    execution_plan_columns = set(
        re.findall(r"(?m)^\s*([a-z_]+)\s+[A-Z]", execution_plans_body.group(1))
    )
    if not {"execution_id", "plan_hash", "status", "summary_json"}.issubset(execution_plan_columns):
        fail("v0.20 execution_plans table missing canonical summary_json columns")
    validate_absent_tokens(migration, "v0.20 migration", ["CREATE TABLE IF NOT EXISTS plan_summaries"])
    validate_absent_tokens(postgres, "v0.20 PostgresStore", ["INSERT INTO plan_summaries", '"plan_summaries"'])
    mapping_validation_text = (SDK_ADAPTER_SRC / "mapping/validation.rs").read_text()
    mapping_validation_fns = rust_fn_names(mapping_validation_text)
    if not {
        "validate_token_id",
        "validate_limit_price_for_sdk",
        "validate_positive_quantity_for_sdk",
    }.issubset(mapping_validation_fns):
        fail("v0.20 SDK mapping validation missing required validation helpers")
    liveness_error_tests = (SDK_ADAPTER_SRC / "tests/liveness_errors.rs").read_text()
    if "normalized_error_redaction_covers_remote_unknown_messages" not in rust_fn_names(liveness_error_tests):
        fail("v0.20 SDK liveness tests missing remote unknown redaction coverage")
    runtime_signal_model_text = (EXECUTOR / "crates/pmx-runtime/src/health/signal/model.rs").read_text()
    runtime_signal_variants = rust_enum_variant_names(runtime_signal_model_text, "RuntimeSignal")
    if not {"Geoblock", "ReconcileBacklog"}.issubset(runtime_signal_variants):
        fail("v0.20 runtime signal model missing blocking runtime signal variants")
    if "remote_unknown_orders" not in rust_enum_variant_field_names(
        runtime_signal_model_text, "RuntimeSignal", "ReconcileBacklog"
    ):
        fail("v0.20 runtime signal model missing remote_unknown_orders backlog field")
    runtime_breakdown_text = (EXECUTOR / "crates/pmx-runtime/src/health/signal/breakdown.rs").read_text()
    if "runtime_breakdown_from_signals" not in rust_fn_names(runtime_breakdown_text):
        fail("v0.20 runtime breakdown missing runtime_breakdown_from_signals")
    breakdown_params, breakdown_return = rust_fn_signature(
        runtime_breakdown_text, "runtime_breakdown_from_signals"
    )
    if "account_id: impl Into<String>" not in breakdown_params or "signals: &[RuntimeSignal]" not in breakdown_params or breakdown_return != "RuntimeHealthBreakdown":
        fail("v0.20 runtime breakdown must bind account_id + &[RuntimeSignal] -> RuntimeHealthBreakdown")
    runtime_breakdown_body = rust_fn_body(
        runtime_breakdown_text, "runtime_breakdown_from_signals"
    )
    require_tokens(
        runtime_breakdown_body,
        "v0.20 runtime breakdown",
        [
            "signal.to_capability_health()",
            "RuntimeSignal::Geoblock",
            "RuntimeSignal::ReconcileBacklog",
            "account_id: account_id.into()",
        ],
    )
    runtime_blocking_tests = (
        EXECUTOR / "crates/pmx-runtime/src/runtime_tests/breakdown_loop/capabilities/blocking.rs"
    ).read_text()
    if "geoblock_unknown_and_reconcile_backlog_block_submit" not in rust_fn_names(runtime_blocking_tests):
        fail("v0.20 runtime evaluation tests missing geoblock/backlog blocking test")
    order_lifecycle_text = (CORE_SRC / "domain/lifecycle/order.rs").read_text()
    if not {"cancel_state_from_lifecycle", "lifecycle_requires_reconcile"}.issubset(
        rust_fn_names(order_lifecycle_text)
    ):
        fail("v0.20 core order lifecycle missing reconcile/cancel helper functions")
    cancel_state_body = rust_fn_body(order_lifecycle_text, "cancel_state_from_lifecycle")
    lifecycle_requires_reconcile_body = rust_fn_body(
        order_lifecycle_text, "lifecycle_requires_reconcile"
    )
    order_lifecycle_states = rust_enum_variant_names(order_lifecycle_text, "OrderLifecycleState")
    if not {"RemoteUnknown", "PartialRemoteUnknown"}.issubset(order_lifecycle_states):
        fail("v0.20 core order lifecycle missing remote unknown states")
    require_tokens(
        cancel_state_body,
        "v0.20 core order lifecycle",
        [
            "OrderLifecycleState::CancelRequested",
            "OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown",
            "crate::CancelState::RemoteUnknown",
        ],
    )
    require_tokens(
        lifecycle_requires_reconcile_body,
        "v0.20 core order lifecycle",
        [
            "OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown",
        ],
    )
    for doc in [
        ROOT / "scripts/package_release.py",
        ROOT / "scripts/check_release_artifact.py",
        EXECUTOR / "validation/check_plan_storage.py",
        EXECUTOR / "validation/run_current_gates.sh",
        EXECUTOR / "docs/PLAN_STORAGE_CANONICALIZATION.md",
        EXECUTOR / "docs/DOC_STATUS.md",
    ]:
        if not doc.exists():
            fail(f"v0.20 missing artifact: {doc.relative_to(ROOT)}")


def validate_v21_sign_only_and_runtime_models(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    expected_sign_only_refs = {
        ("/v1/sign-only/lifecycle-events", "post", "request"): "#/components/schemas/SignOnlyLifecycleRecord",
        ("/v1/sign-only/lifecycle-events", "post", "202"): "#/components/schemas/SignOnlyLifecycleRecord",
        ("/v1/sign-only/standard-constructions", "post", "request"): "#/components/schemas/StandardSignOnlyConstructionRequest",
        ("/v1/sign-only/standard-constructions", "post", "202"): "#/components/schemas/StandardSignOnlyConstructionReceipt",
        ("/v1/sign-only/lifecycle-events/{execution_id}", "get", "200_items"): "#/components/schemas/SignOnlyLifecycleRecord",
    }
    for (path, method, kind), expected_ref in expected_sign_only_refs.items():
        if kind == "request":
            actual_ref = operation_request_ref(spec, path, method)
        elif kind == "200_items":
            actual_ref = operation_response_array_item_ref(spec, path, method, "200")
        else:
            actual_ref = operation_response_ref(spec, path, method, kind)
        if actual_ref != expected_ref:
            fail(f"v0.21 {method.upper()} {path} {kind} must reference {expected_ref}, got {actual_ref}")
    sign_only_required = schema_required_names(spec, "SignOnlyLifecycleRecord")
    if not {"execution_id", "account_id", "state", "event", "signed_order_ref", "no_remote_side_effect"}.issubset(sign_only_required):
        fail("v0.21 SignOnlyLifecycleRecord schema missing required lifecycle fields")
    sign_only_props = schema_property_names(spec, "SignOnlyLifecycleRecord")
    for required_prop in {"client_event_id", "signed_order_ref", "no_remote_side_effect"}:
        if required_prop not in sign_only_props:
            fail(f"v0.21 SignOnlyLifecycleRecord schema must expose {required_prop}")
    standard_req_required = schema_required_names(spec, "StandardSignOnlyConstructionRequest")
    if not {"execution_id", "account_id", "plan_hash", "no_remote_side_effect"}.issubset(standard_req_required):
        fail("v0.21 StandardSignOnlyConstructionRequest missing required binding fields")
    standard_req_props = schema_property_names(spec, "StandardSignOnlyConstructionRequest")
    for required_prop in {"signed_order_ref", "signed_order_digest", "no_remote_side_effect"}:
        if required_prop not in standard_req_props:
            fail(f"v0.21 StandardSignOnlyConstructionRequest must expose {required_prop}")
    standard_receipt_props = schema_property_names(spec, "StandardSignOnlyConstructionReceipt")
    for required_prop in {"signed_order_ref", "signed_order_digest", "lifecycle_records", "no_remote_side_effect"}:
        if required_prop not in standard_receipt_props:
            fail(f"v0.21 StandardSignOnlyConstructionReceipt must expose {required_prop}")
    runtime_worker_props = schema_property_names(spec, "RuntimeWorkerStatusReport")
    if runtime_worker_props != {"heartbeats", "observations"}:
        fail(f"v0.21 RuntimeWorkerStatusReport properties changed unexpectedly: {sorted(runtime_worker_props)}")
    sign_only_core_text = (CORE_SRC / "domain/lifecycle/sign_only.rs").read_text()
    try:
        sign_only_state_variants = rust_enum_variant_names(
            sign_only_core_text, "SignOnlyLifecycleState"
        )
        sign_only_event_variants = rust_enum_variant_names(
            sign_only_core_text, "SignOnlyLifecycleEventKind"
        )
        sign_only_record_fields = rust_struct_field_names(
            sign_only_core_text, "SignOnlyLifecycleRecord"
        )
    except SystemExit as exc:
        fail(f"v0.21 sign-only lifecycle core malformed: {exc}")
    if sign_only_state_variants != {
        "Planned",
        "ReservationPrepared",
        "SigningRequested",
        "SignedDryRun",
        "Failed",
        "Abandoned",
    }:
        fail("v0.21 sign-only lifecycle core missing SignOnlyLifecycleState variants")
    if sign_only_event_variants != {
        "PrepareReservation",
        "RequestSigning",
        "SignedWithoutPost",
        "SigningFailed",
        "Abandon",
    }:
        fail("v0.21 sign-only lifecycle core missing SignOnlyLifecycleEventKind variants")
    if not {
        "execution_id",
        "account_id",
        "state",
        "event",
        "client_event_id",
        "signed_order_ref",
        "no_remote_side_effect",
        "event_id",
        "created_at",
    }.issubset(sign_only_record_fields):
        fail("v0.21 sign-only lifecycle core missing SignOnlyLifecycleRecord fields")
    require_tokens(
        sign_only_core_text,
        "v0.21 sign-only lifecycle core",
        [
            "transition_sign_only_lifecycle",
            "sign_only_lifecycle_has_remote_side_effect",
            "sign_only_lifecycle_records_equivalent",
        ],
    )
    lifecycle_adapter_text = (SDK_ADAPTER_SRC / "lifecycle.rs").read_text()
    sign_only_tests_text = (SDK_ADAPTER_SRC / "tests/sign_only.rs").read_text()
    lifecycle_fns = rust_fn_names(lifecycle_adapter_text)
    sign_only_test_fns = rust_fn_names(sign_only_tests_text) | rust_async_fn_names(
        sign_only_tests_text
    )
    if lifecycle_fns != {"sign_only_lifecycle_records_from_receipt"}:
        fail("v0.21 sign-only lifecycle adapter missing conversion helper")
    lifecycle_params, lifecycle_return = rust_fn_signature(
        lifecycle_adapter_text, "sign_only_lifecycle_records_from_receipt"
    )
    if "&SignOnlyDryRunReceipt" not in lifecycle_params or "Result<Vec<SignOnlyLifecycleRecord>, OfficialSdkAdapterError>" not in lifecycle_return:
        fail("v0.21 sign-only lifecycle adapter must bind &SignOnlyDryRunReceipt -> Result<Vec<SignOnlyLifecycleRecord>, OfficialSdkAdapterError>")
    lifecycle_body = rust_fn_body(lifecycle_adapter_text, "sign_only_lifecycle_records_from_receipt")
    require_tokens(
        lifecycle_body,
        "v0.21 sign-only lifecycle adapter",
        [
            "transition_sign_only_lifecycle",
            "no_remote_side_effect: true",
            "sign-only receipt unexpectedly indicates remote posting",
            "signed_order_ref: Some(receipt.signed_order_ref.clone())",
        ],
    )
    if not {
        "sign_only_lifecycle_records_are_persistable_and_non_mutating",
        "sign_only_lifecycle_rejects_posted_receipt",
        "standard_sign_only_construction_emits_only_digest_ref_and_lifecycle",
    }.issubset(sign_only_test_fns):
        fail("v0.21 sign-only lifecycle tests missing required test functions")
    runtime_action_text = (
        EXECUTOR / "crates/pmx-runtime/src/health/action.rs"
    ).read_text()
    runtime_worker_text = (
        EXECUTOR / "crates/pmx-runtime/src/health/worker.rs"
    ).read_text()
    try:
        worker_action_fields = rust_struct_field_names(
            runtime_action_text, "RuntimeWorkerAction"
        )
        worker_store_write_fields = rust_struct_field_names(
            runtime_action_text, "RuntimeWorkerStoreWrite"
        )
        runtime_worker_kind_variants = rust_enum_variant_names(
            runtime_worker_text, "RuntimeWorkerKind"
        )
    except SystemExit as exc:
        fail(f"v0.21 runtime worker models malformed: {exc}")
    if worker_action_fields != {
        "kind",
        "capability",
        "should_fail_closed",
        "should_update_runtime_store",
        "reason",
    }:
        fail("v0.21 runtime worker action model missing RuntimeWorkerAction fields")
    if worker_store_write_fields != {
        "account_id",
        "capability",
        "worker_kind",
        "status",
        "should_fail_closed",
        "reason",
    }:
        fail("v0.21 runtime worker action model missing RuntimeWorkerStoreWrite fields")
    if runtime_worker_kind_variants != {
        "WebSocketLiveness",
        "HeartbeatLease",
        "Geoblock",
        "ResourceRefresh",
        "ReconcileBacklog",
    }:
        fail("v0.21 runtime worker kinds missing RuntimeWorkerKind variants")
    worker_actions_params, worker_actions_return = rust_fn_signature(
        runtime_action_text, "worker_actions_from_runtime_signals"
    )
    if "signals: &[RuntimeSignal]" not in worker_actions_params or worker_actions_return != "Vec<RuntimeWorkerAction>":
        fail("v0.21 runtime worker action model must bind &[RuntimeSignal] -> Vec<RuntimeWorkerAction>")
    runtime_store_write_params, runtime_store_write_return = rust_fn_signature(
        runtime_action_text, "runtime_worker_store_writes"
    )
    if "account_id: impl Into<String>" not in runtime_store_write_params or "signals: &[RuntimeSignal]" not in runtime_store_write_params or runtime_store_write_return != "Vec<RuntimeWorkerStoreWrite>":
        fail("v0.21 runtime worker action model must bind account_id + &[RuntimeSignal] -> Vec<RuntimeWorkerStoreWrite>")
    worker_actions_body = rust_fn_body(runtime_action_text, "worker_actions_from_runtime_signals")
    runtime_store_writes_body = rust_fn_body(runtime_action_text, "runtime_worker_store_writes")
    require_tokens(
        worker_actions_body,
        "v0.21 runtime worker action model",
        [
            "signal.to_capability_health()",
            "signal.worker_kind()",
            "should_fail_closed: health.blocks_submit()",
        ],
    )
    require_tokens(
        runtime_store_writes_body,
        "v0.21 runtime worker action model",
        [
            "account_id = account_id.into()",
            "should_fail_closed = health.blocks_submit()",
            "RuntimeWorkerStoreWrite",
        ],
    )
    runtime_worker_tests_text = (
        EXECUTOR
        / "crates/pmx-runtime/src/runtime_tests/breakdown_loop/capabilities/groups.rs"
    ).read_text()
    runtime_worker_test_fns = rust_fn_names(runtime_worker_tests_text) | rust_async_fn_names(
        runtime_worker_tests_text
    )
    if "worker_actions_mark_stale_runtime_inputs_as_fail_closed_updates" not in runtime_worker_test_fns:
        fail("v0.21 runtime worker tests missing required test function")
    for doc in [
        EXECUTOR / "validation/check_sign_only_lifecycle.py",
        EXECUTOR / "validation/check_runtime_worker_models.py",
        EXECUTOR / "validation/run_current_gates.sh",
        EXECUTOR / "docs/SIGN_ONLY_LIFECYCLE_PERSISTENCE.md",
        EXECUTOR / "docs/RUNTIME_WORKER_MODEL.md",
        EXECUTOR / "docs/DOC_STATUS.md",
    ]:
        if not doc.exists():
            fail(f"v0.21 missing artifact: {doc.relative_to(ROOT)}")


def validate_store_and_backend_structure() -> None:
    store_lib = (STORE_SRC / "lib.rs").read_text()
    postgres_rs = (STORE_SRC / "postgres.rs").read_text()
    service_lib = (SERVICE_SRC / "lib.rs").read_text()
    api_backend_audit = (API_SRC / "backend/audit.rs").read_text()
    api_backend_sign_only = (API_SRC / "backend/sign_only.rs").read_text()
    api_backend_runtime = (API_SRC / "backend/runtime.rs").read_text()
    store_private_modules = rust_module_names(store_lib, "mod")
    for module_name in [
        "postgres_audit",
        "postgres_execution",
        "postgres_idempotency",
        "postgres_runtime",
        "postgres_sign_only",
        "postgres_worker",
    ]:
        if module_name not in store_private_modules:
            fail(f"pmx-store module boundary missing token: mod {module_name};")
    if "postgres" not in rust_module_names(store_lib, "pub mod"):
        fail("pmx-store module boundary missing token: pub mod postgres;")
    if "postgres" not in rust_pub_use_targets(store_lib):
        fail("pmx-store module boundary missing token: pub use postgres::PostgresStore;")
    try:
        postgres_store_fields = rust_struct_field_names(postgres_rs, "PostgresStore")
    except SystemExit as exc:
        fail(f"PostgresStore structure malformed: {exc}")
    postgres_store_methods = rust_fn_names(postgres_rs) | rust_async_fn_names(postgres_rs)
    if postgres_store_fields != {"database_url"}:
        fail("PostgresStore structure missing canonical database_url field")

    for fn_name in ["new", "connect", "apply_schema", "applied_schema_migrations", "client", "rollback"]:
        if fn_name not in postgres_store_methods:
            fail(f"PostgresStore structure missing impl method: {fn_name}")
    connect_body = rust_impl_method_body(postgres_rs, "PostgresStore", "connect")
    client_body = rust_impl_method_body(postgres_rs, "PostgresStore", "client")
    rollback_body = rust_impl_method_body(postgres_rs, "PostgresStore", "rollback")
    if 'simple_query("SELECT 1")' not in connect_body:
        fail('PostgresStore::connect must probe the database with simple_query("SELECT 1")')
    if "tokio_postgres::connect(&self.database_url, NoTls)" not in client_body:
        fail("PostgresStore::client must connect with tokio_postgres::connect(&self.database_url, NoTls)")
    if 'client.batch_execute("ROLLBACK")' not in rollback_body:
        fail('PostgresStore::rollback must issue client.batch_execute("ROLLBACK")')

    service_modules = rust_module_names(service_lib, "mod")
    for module_name in ["runtime_state", "runtime_worker", "sign_only", "submit"]:
        if module_name not in service_modules:
            fail(f"pmx-service module boundary missing token: mod {module_name};")
    service_reexports = rust_pub_use_targets(service_lib)
    for module_name in ["runtime_state", "runtime_worker", "sign_only", "submit"]:
        if module_name not in service_reexports:
            fail(f"pmx-service module boundary missing token: pub use {module_name}::*;")
    api_backend_audit_impl_methods = service_backend_method_names(api_backend_audit)
    api_backend_sign_only_impl_methods = service_backend_method_names(api_backend_sign_only)
    api_backend_runtime_impl_methods = service_backend_method_names(api_backend_runtime)
    if not {"record_admin_audit_event", "list_admin_audit_events"}.issubset(
        api_backend_audit_impl_methods
    ):
        fail("pmx-api audit backend bridge missing ServiceBackend impl methods")
    if not {"record_standard_sign_only_construction", "list_sign_only_lifecycle_events"}.issubset(
        api_backend_sign_only_impl_methods
    ):
        fail("pmx-api sign-only backend bridge missing ServiceBackend impl methods")
    if not {"list_runtime_worker_status", "set_account_kill_switch", "set_global_kill_switch"}.issubset(
        api_backend_runtime_impl_methods
    ):
        fail("pmx-api runtime backend bridge missing ServiceBackend impl methods")

    ensure_match_arms(
        api_backend_audit,
        "pmx-api audit backend bridge",
        "record_admin_audit_event",
        [
            "Self::InMemory(service) => service.record_admin_audit_event(event).await",
            "Self::Postgres(service) => service.record_admin_audit_event(event).await",
        ],
    )
    ensure_match_arms(
        api_backend_audit,
        "pmx-api audit backend bridge",
        "list_admin_audit_events",
        [
            "Self::InMemory(service) => service.list_admin_audit_events(query).await",
            "Self::Postgres(service) => service.list_admin_audit_events(query).await",
        ],
    )

    ensure_match_arms(
        api_backend_sign_only,
        "pmx-api sign-only backend bridge",
        "record_standard_sign_only_construction",
        [
            "Self::InMemory(service) => service.record_standard_sign_only_construction(req).await",
            "Self::Postgres(service) => service.record_standard_sign_only_construction(req).await",
        ],
    )
    ensure_match_arms(
        api_backend_sign_only,
        "pmx-api sign-only backend bridge",
        "list_sign_only_lifecycle_events",
        [
            "Self::InMemory(service) => service.list_sign_only_lifecycle_events(query).await",
            "Self::Postgres(service) => service.list_sign_only_lifecycle_events(query).await",
        ],
    )

    ensure_match_arms(
        api_backend_runtime,
        "pmx-api runtime backend bridge",
        "list_runtime_worker_status",
        [
            "Self::InMemory(service) => service.list_runtime_worker_status(query).await",
            "Self::Postgres(service) => service.list_runtime_worker_status(query).await",
        ],
    )
    ensure_match_arms(
        api_backend_runtime,
        "pmx-api runtime backend bridge",
        "set_account_kill_switch",
        [
            ".store()",
            ".set_account_kill_switch(account_id, enabled, reason)",
            "Self::InMemory(service)",
            "Self::Postgres(service)",
        ],
    )
    ensure_match_arms(
        api_backend_runtime,
        "pmx-api runtime backend bridge",
        "set_global_kill_switch",
        [
            ".store()",
            ".set_global_kill_switch(enabled, reason)",
            "Self::InMemory(service)",
            "Self::Postgres(service)",
        ],
    )


def validate_v23_lifecycle_query_and_hardening(spec: dict | None = None) -> None:
    if spec is None:
        import yaml

        spec = yaml.safe_load(OPENAPI.read_text())
    core = rust_source_text(CORE_SRC)
    store = rust_source_text(STORE_SRC)
    postgres = rust_source_text(STORE_SRC)
    service = rust_source_text(SERVICE_SRC)
    policy = (EXECUTOR / "crates/pmx-policy/src/lib.rs").read_text()
    sql = SQL.read_text()
    gate = (EXECUTOR / "validation/run_current_gates_impl.sh").read_text()
    core_redaction = (CORE_SRC / "domain/plan/redaction.rs").read_text()
    core_sign_only = (CORE_SRC / "domain/lifecycle/sign_only.rs").read_text()
    store_order_lifecycle = (STORE_SRC / "model/order_lifecycle.rs").read_text()
    store_runtime = (STORE_SRC / "model/runtime.rs").read_text()
    store_audit = (STORE_SRC / "model/audit.rs").read_text()
    api_runtime_read = (API_SRC / "routes/read/runtime.rs").read_text()
    api_lifecycle_read = (API_SRC / "routes/read/lifecycle.rs").read_text()
    api_sign_only_flow = (API_SRC / "routes/flow/sign_only.rs").read_text()
    api_admin_audit = (API_SRC / "routes/admin/audit.rs").read_text()
    api_reconcile_local = (API_SRC / "routes/admin/reconcile/local.rs").read_text()
    api_support_error = (API_SRC / "support/error.rs").read_text()
    api_support_audit = (API_SRC / "support/audit.rs").read_text()
    api_cancel_route = (API_SRC / "routes/admin/cancel.rs").read_text()
    required_paths = {
        "/v1/sign-only/lifecycle-events",
        "/v1/lifecycle/executions/{execution_id}/events",
        "/v1/runtime/workers",
        "/v1/admin/audit-events",
        "/v1/admin/reconcile-order-local",
    }
    missing_paths = required_paths - set(spec["paths"].keys())
    if missing_paths:
        fail(f"current OpenAPI missing lifecycle/runtime/admin paths: {sorted(missing_paths)}")

    sign_only_events = openapi_operation(spec, "/v1/sign-only/lifecycle-events/{execution_id}", "get")
    execution_events = openapi_operation(spec, "/v1/lifecycle/executions/{execution_id}/events", "get")
    audit_events = openapi_operation(spec, "/v1/admin/audit-events", "get")
    reconcile_local = openapi_operation(spec, "/v1/admin/reconcile-order-local", "post")
    runtime_workers = openapi_operation(spec, "/v1/runtime/workers", "get")

    if "before_event_id" not in operation_parameter_names(sign_only_events):
        fail("current OpenAPI sign-only lifecycle execution query must expose before_event_id")
    if "before_event_id" not in operation_parameter_names(execution_events):
        fail("current OpenAPI execution lifecycle query must expose before_event_id")
    if "before_audit_id" not in operation_parameter_names(audit_events):
        fail("current OpenAPI admin audit query must expose before_audit_id")

    response_ref = (
        runtime_workers.get("responses", {})
        .get("200", {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("$ref")
    )
    if response_ref != "#/components/schemas/RuntimeWorkerStatusReport":
        fail("current OpenAPI runtime worker response must reference RuntimeWorkerStatusReport")

    request_ref = (
        reconcile_local.get("requestBody", {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("$ref")
    )
    if request_ref != "#/components/schemas/ReconcileOrderLocalRequest":
        fail("current OpenAPI reconcile-order-local request must reference ReconcileOrderLocalRequest")
    reconcile_response_ref = (
        reconcile_local.get("responses", {})
        .get("202", {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
        .get("$ref")
    )
    if reconcile_response_ref != "#/components/schemas/ReconcileOrderLocalResponse":
        fail("current OpenAPI reconcile-order-local response must reference ReconcileOrderLocalResponse")

    if "client_event_id" not in schema_property_names(spec, "SignOnlyLifecycleRecord"):
        fail("current OpenAPI SignOnlyLifecycleRecord must expose client_event_id")
    try:
        redacted_payload_fields = rust_struct_field_names(
            core_redaction, "RedactedPayloadEnvelope"
        )
        sign_only_record_fields = rust_struct_field_names(
            core_sign_only, "SignOnlyLifecycleRecord"
        )
        order_lifecycle_fields = rust_struct_field_names(
            store_order_lifecycle, "OrderLifecycleRecord"
        )
        runtime_observation_fields = rust_struct_field_names(
            store_runtime, "RuntimeWorkerObservation"
        )
        runtime_status_report_fields = rust_struct_field_names(
            store_runtime, "RuntimeWorkerStatusReport"
        )
        admin_audit_event_fields = rust_struct_field_names(
            store_audit, "AdminAuditEvent"
        )
        admin_audit_query_fields = rust_struct_field_names(
            store_audit, "AdminAuditQuery"
        )
        order_lifecycle_store_methods = rust_trait_method_signatures(
            store_order_lifecycle, "OrderLifecycleStore"
        )
        runtime_worker_health_methods = rust_trait_method_signatures(
            store_runtime, "RuntimeWorkerHealthStore"
        )
        runtime_worker_status_methods = rust_trait_method_signatures(
            store_runtime, "RuntimeWorkerStatusStore"
        )
    except SystemExit as exc:
        fail(f"current lifecycle/runtime/store models malformed: {exc}")
    if not {
        "schema_version",
        "kind",
        "correlation_id",
        "redacted_fields",
        "body",
    }.issubset(redacted_payload_fields):
        fail("current RedactedPayloadEnvelope must expose redacted payload fields")
    if not {
        "execution_id",
        "account_id",
        "state",
        "event",
        "client_event_id",
        "signed_order_ref",
        "no_remote_side_effect",
        "event_id",
        "created_at",
    }.issubset(sign_only_record_fields):
        fail("current SignOnlyLifecycleRecord must expose replay-safe lifecycle fields")
    if not {
        "order_id",
        "execution_id",
        "account_id",
        "condition_id",
        "token_id",
        "side",
        "lifecycle_state",
        "remote_order_id",
        "remote_state",
        "created_at",
        "updated_at",
    }.issubset(order_lifecycle_fields):
        fail("current OrderLifecycleRecord must expose lifecycle persistence fields")
    if not {
        "account_id",
        "capability",
        "worker_kind",
        "status",
        "should_fail_closed",
        "reason",
        "observed_at",
    }.issubset(runtime_observation_fields):
        fail("current RuntimeWorkerObservation must expose observation freshness fields")
    if runtime_status_report_fields != {"heartbeats", "observations"}:
        fail("current RuntimeWorkerStatusReport must expose heartbeats and observations")
    if not {
        "audit_id",
        "principal_subject",
        "operation",
        "request_fingerprint",
        "correlation_id",
        "result",
        "created_at",
    }.issubset(admin_audit_event_fields):
        fail("current AdminAuditEvent must expose audit result and correlation fields")
    if not {
        "limit",
        "before_audit_id",
        "operation",
        "principal_subject",
        "result",
        "correlation_id",
    }.issubset(admin_audit_query_fields):
        fail("current AdminAuditQuery must expose audit pagination and filters")
    if order_lifecycle_store_methods != {
        "upsert_order_lifecycle",
        "record_order_lifecycle_event",
        "load_order_lifecycle",
        "list_order_lifecycle_events",
    }:
        fail("current OrderLifecycleStore must expose lifecycle persistence operations")
    if runtime_worker_health_methods != {"record_worker_heartbeat"}:
        fail("current RuntimeWorkerHealthStore must expose record_worker_heartbeat")
    if runtime_worker_status_methods != {"list_runtime_worker_status"}:
        fail("current RuntimeWorkerStatusStore must expose list_runtime_worker_status")
    if "redacted_payload_envelope" not in rust_fn_names(core_redaction):
        fail("current core redaction must expose redacted_payload_envelope")
    if "sign_only_lifecycle_records_equivalent" not in rust_fn_names(core_sign_only):
        fail("current sign-only lifecycle core must expose replay equivalence helper")
    if "correlation_id_from_headers" not in rust_fn_names(api_support_error):
        fail("current API error support must expose correlation_id_from_headers")
    if "api_error_with_correlation" not in rust_fn_names(api_support_error):
        fail("current API error support must expose api_error_with_correlation")
    if "record_admin_audit" not in rust_async_fn_names(api_support_audit):
        fail("current API audit support must expose async record_admin_audit")
    if "list_runtime_worker_status" not in rust_async_fn_names(api_runtime_read):
        fail("current API runtime read route must expose list_runtime_worker_status")
    if "reconcile_order_local" not in rust_async_fn_names(api_reconcile_local):
        fail("current API reconcile route must expose reconcile_order_local")
    if "record_cancel_order_non_live" not in rust_async_fn_names(api_cancel_route):
        fail("current API cancel route must expose record_cancel_order_non_live")
    if "list_admin_audit_events" not in rust_async_fn_names(api_admin_audit):
        fail("current API admin audit route must expose list_admin_audit_events")
    if not {
        "record_sign_only_lifecycle_event",
        "record_standard_sign_only_construction",
    }.issubset(rust_async_fn_names(api_sign_only_flow)):
        fail("current API sign-only flow routes must expose persistence and construction endpoints")
    if not {
        "list_sign_only_lifecycle_events",
        "list_execution_lifecycle_events",
        "list_order_lifecycle_events",
    }.issubset(rust_async_fn_names(api_lifecycle_read)):
        fail("current API lifecycle read routes must expose list endpoints")
    try:
        runtime_params, runtime_return = rust_async_fn_signature(
            api_runtime_read, "list_runtime_worker_status"
        )
        sign_only_params, sign_only_return = rust_async_fn_signature(
            api_lifecycle_read, "list_sign_only_lifecycle_events"
        )
        execution_params, execution_return = rust_async_fn_signature(
            api_lifecycle_read, "list_execution_lifecycle_events"
        )
        order_params, order_return = rust_async_fn_signature(
            api_lifecycle_read, "list_order_lifecycle_events"
        )
        sign_only_flow_params, sign_only_flow_return = rust_async_fn_signature(
            api_sign_only_flow, "record_sign_only_lifecycle_event"
        )
        construction_params, construction_return = rust_async_fn_signature(
            api_sign_only_flow, "record_standard_sign_only_construction"
        )
        reconcile_params, reconcile_return = rust_async_fn_signature(
            api_reconcile_local, "reconcile_order_local"
        )
    except SystemExit as exc:
        fail(f"current API route signatures malformed: {exc}")
    if "Query(query): Query<RuntimeWorkerStatusListQuery>" not in runtime_params or runtime_return != "ApiResult<RuntimeWorkerStatusReport>":
        fail("current API runtime read route must bind RuntimeWorkerStatusListQuery -> ApiResult<RuntimeWorkerStatusReport>")
    if "Query(query): Query<EventListQuery>" not in sign_only_params or sign_only_return != "ApiResult<Vec<SignOnlyLifecycleRecord>>":
        fail("current API sign-only lifecycle read route must bind EventListQuery -> ApiResult<Vec<SignOnlyLifecycleRecord>>")
    if "Query(query): Query<EventListQuery>" not in execution_params or execution_return != "ApiResult<Vec<ExecutionLifecycleEvent>>":
        fail("current API execution lifecycle read route must bind EventListQuery -> ApiResult<Vec<ExecutionLifecycleEvent>>")
    if "Query(query): Query<EventListQuery>" not in order_params or order_return != "ApiResult<Vec<OrderLifecycleEventRecord>>":
        fail("current API order lifecycle read route must bind EventListQuery -> ApiResult<Vec<OrderLifecycleEventRecord>>")
    if "Json(record): Json<SignOnlyLifecycleRecord>" not in sign_only_flow_params or sign_only_flow_return != "ApiResult<SignOnlyLifecycleRecord>":
        fail("current API sign-only lifecycle record route must bind Json<SignOnlyLifecycleRecord> -> ApiResult<SignOnlyLifecycleRecord>")
    if "Json(req): Json<StandardSignOnlyConstructionRequest>" not in construction_params or construction_return != "ApiResult<StandardSignOnlyConstructionReceipt>":
        fail("current API sign-only construction route must bind Json<StandardSignOnlyConstructionRequest> -> ApiResult<StandardSignOnlyConstructionReceipt>")
    if "Json(req): Json<ReconcileOrderLocalRequest>" not in reconcile_params or reconcile_return != "ApiResult<ReconcileOrderLocalResponse>":
        fail("current API reconcile-order-local route must bind Json<ReconcileOrderLocalRequest> -> ApiResult<ReconcileOrderLocalResponse>")
    required_by_file = {
        "core": (core, ["WorkerDegraded", "left.client_event_id == right.client_event_id"]),
        "store": (store, ["in_memory_order_lifecycle_records_cancel_requested", "in_memory_worker_heartbeat_informs_runtime_state", "sign_only_lifecycle_record_is_replay", "client_event_id reused with different event payload", "PMX_RUNTIME_OBSERVATION_TTL_SECONDS", "runtime_observation_ttl_seconds", "execution_id={}"]),
        "postgres": (postgres, ["impl OrderLifecycleStore for PostgresStore", "postgres_records_order_lifecycle_event", "impl RuntimeWorkerHealthStore for PostgresStore", "impl RuntimeWorkerStatusStore for PostgresStore", "postgres_records_worker_heartbeat", "postgres_lists_runtime_worker_status", "principal_subject = $4", "result = $5", "pg_advisory_xact_lock", "sign_only_lifecycle_record_is_replay", "runtime_observation_ttl_seconds", "FOREIGN_KEY_VIOLATION", "CHECK_VIOLATION"]),
        "sql": (sql, ["CREATE TABLE IF NOT EXISTS orders", "CREATE TABLE IF NOT EXISTS order_events", "idx_order_events_order_created", "client_event_id TEXT", "uq_sign_only_lifecycle_client_event", "WHERE client_event_id IS NOT NULL", "ADD COLUMN IF NOT EXISTS client_event_id", "ADD COLUMN IF NOT EXISTS observed_at", "ADD COLUMN IF NOT EXISTS correlation_id"]),
        "service": (service, ["candidate.client_event_id.as_deref()", "record.event_id = None", "record.created_at = None", "account_id does not match request"]),
        "policy": (policy, ["WorkerStatus::Degraded => reasons.push(BlockReason::WorkerDegraded)", "degraded_worker_blocks_pre_live"]),
        "api runtime read route": (api_runtime_read, ["Query(query): Query<RuntimeWorkerStatusListQuery>", "list_runtime_worker_status(RuntimeWorkerStatusQuery", "StatusCode::OK"]),
        "api reconcile local route": (api_reconcile_local, ["api_error_with_correlation", "record_admin_audit(", "ReconcileOrderLocalResponse", "no_remote_side_effect: true"]),
        "api support error": (api_support_error, []),
        "api support audit": (api_support_audit, ["operation: &'static str", "AdminAuditEvent"]),
        "api cancel route": (api_cancel_route, ["redacted_payload_envelope", "payload: redacted_payload_envelope(", "CancelReceipt"]),
        "gate": (gate, ["run_current_gates.sh", "check_current_lifecycle_api.py", "check_version_consistency.py", "check_docs_evidence_governance.py", "write_current_evidence_manifest.py", "check_runtime_worker_status_query.py", "42-runtime-worker-status-query.log", "evidence/current"]),
    }
    validate_required_groups(required_by_file)
    if core.count("pub client_event_id: Option<String>") != 1:
        fail("current SignOnlyLifecycleRecord must have exactly one client_event_id field")
    if store.count("pub observed_at: Option<DateTime<Utc>>") != 1:
        fail("current RuntimeWorkerObservation must have exactly one observed_at field")
    openapi_text = OPENAPI.read_text()
    if "SignedOrderEnvelope" in openapi_text or "signed_payload" in openapi_text:
        fail("current public OpenAPI must not expose signed payload internals")
