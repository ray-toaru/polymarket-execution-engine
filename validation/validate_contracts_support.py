from __future__ import annotations

import importlib
import importlib.util
import ast
import re
import sys
from pathlib import Path


VALIDATION_DIR = Path(__file__).resolve().parent
ENGINE_ROOT = VALIDATION_DIR.parent
ROOT = ENGINE_ROOT.parent
INTEGRATION_SCRIPT_DIR = ROOT / "scripts"
if str(VALIDATION_DIR) not in sys.path:
    sys.path.insert(0, str(VALIDATION_DIR))
if str(INTEGRATION_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(INTEGRATION_SCRIPT_DIR))

from release_policy import EXCLUDED_PREFIXES

EXECUTOR = ROOT / "polymarket-execution-engine"
CONTROL = ROOT / "hermes-polymarket-executor-adapter"
OPENAPI = EXECUTOR / "openapi" / "executor.v1.yaml"
API_SRC = EXECUTOR / "crates" / "pmx-api" / "src"
CORE_SRC = EXECUTOR / "crates" / "pmx-core" / "src"
STORE_SRC = EXECUTOR / "crates" / "pmx-store" / "src"
SERVICE_SRC = EXECUTOR / "crates" / "pmx-service" / "src"
API_RS = API_SRC / "lib.rs"
SQL = EXECUTOR / "migrations" / "0001_initial.sql"
STORE_RS = STORE_SRC / "lib.rs"
POSTGRES_RS = EXECUTOR / "crates/pmx-store/src/postgres.rs"
API_E2E_TEST = EXECUTOR / "crates/pmx-api/tests/http_and_fake_e2e.rs"
API_POSTGRES_E2E_TEST = EXECUTOR / "crates/pmx-api/tests/http_postgres_e2e.rs"
GATEWAY_SRC = EXECUTOR / "crates/pmx-gateway/src"
SDK_SPIKE_RS = EXECUTOR / "adapters/pmx-official-sdk-spike/src/lib.rs"
SDK_SPIKE_TOML = EXECUTOR / "adapters/pmx-official-sdk-spike/Cargo.toml"
SDK_ADAPTER_RS = EXECUTOR / "adapters/pmx-official-sdk-adapter/src/lib.rs"
SDK_ADAPTER_SRC = EXECUTOR / "adapters/pmx-official-sdk-adapter/src"
SDK_ADAPTER_TOML = EXECUTOR / "adapters/pmx-official-sdk-adapter/Cargo.toml"
LIVE_SUBMIT_GUARD = EXECUTOR / "validation/check_live_submit_guard.py"
SERVICE_RS = SERVICE_SRC / "lib.rs"
SERVICE_TOML = EXECUTOR / "crates/pmx-service/Cargo.toml"
ROOT_CARGO_TOML = EXECUTOR / "Cargo.toml"

FORBIDDEN_PUBLIC_TOKEN_PATTERNS = {
    "SignedOrderEnvelope": re.compile(r"\bSignedOrderEnvelope\b"),
    "signed_payload": re.compile(r"\bsigned[_-]?payload\b", re.IGNORECASE),
    "raw_signed_payload": re.compile(r"\braw[_-]?signed[_-]?payload\b", re.IGNORECASE),
    "raw_signature": re.compile(r"\braw[_-]?signature\b", re.IGNORECASE),
    "private_key": re.compile(r"\bprivate[_-]?key\b", re.IGNORECASE),
    "clob_secret": re.compile(r"\bclob[_-]?secret\b", re.IGNORECASE),
    "api_secret": re.compile(r"\bapi[_-]?secret\b", re.IGNORECASE),
    "api_passphrase": re.compile(r"\bapi[_-]?passphrase\b", re.IGNORECASE),
}

PUBLIC_CONTRACT_SOURCE_PATHS = [
    CONTROL / "src",
    API_SRC / "model.rs",
    API_SRC / "lib.rs",
]

EXPECTED_202_PATHS = {
    "/v1/submissions": "submit_plan",
    "/v1/sign-only/standard-constructions": "record_standard_sign_only_construction",
    "/v1/admin/kill-switch": "set_kill_switch",
    "/v1/admin/cancel-order": "record_cancel_order_non_live",
    "/v1/admin/reconcile": "record_reconcile_non_live",
    "/v1/admin/reconcile-order-local": "reconcile_order_local",
}

PY_MODEL_BY_SCHEMA = {
    "MarketRef": "MarketRef",
    "QuantityIntent": "QuantityIntent",
    "TradeIntent": "TradeIntent",
    "NormalizedIntent": "NormalizedIntent",
    "RuntimeStateSummary": "RuntimeStateSummary",
    "FeasibilitySnapshot": "FeasibilitySnapshot",
    "ConstraintDecision": "ConstraintDecision",
    "ApprovalReceipt": "ApprovalReceipt",
    "ExecutionPlanSummary": "ExecutionPlanSummary",
    "SubmitReceipt": "SubmitReceipt",
    "CancelReceipt": "CancelReceipt",
    "KillSwitchReceipt": "KillSwitchReceipt",
    "ReconcileReport": "ReconcileReport",
    "OrderLifecycleRecord": "OrderLifecycleRecord",
    "OrderLifecycleDivergence": "OrderLifecycleDivergence",
    "ReconcileOrderLocalResponse": "ReconcileOrderLocalResponse",
    "SignOnlyLifecycleRecord": "SignOnlyLifecycleRecord",
    "StandardSignOnlyConstructionRequest": "StandardSignOnlyConstructionRequest",
    "StandardSignOnlyConstructionReceipt": "StandardSignOnlyConstructionReceipt",
    "RedactedPayloadEnvelope": "RedactedPayloadEnvelope",
    "ExecutionLifecycleEvent": "ExecutionLifecycleEvent",
    "AdminAuditEvent": "AdminAuditEvent",
    "HealthReport": "HealthReport",
}


class ContractValidationError(Exception):
    pass


def fail(message: str) -> None:
    raise ContractValidationError(f"contract validation failed: {message}")


def normalize_path(path: str) -> str:
    return re.sub(r":([A-Za-z_][A-Za-z0-9_]*)", r"{\1}", path)


def rust_source_text(src: Path) -> str:
    return "\n".join(path.read_text() for path in sorted(src.rglob("*.rs")))


def rust_file_with_modules_text(src: Path) -> str:
    texts: list[str] = []
    if src.exists():
        texts.append(src.read_text())
    module_dir = src.with_suffix("")
    if module_dir.is_dir():
        texts.extend(path.read_text() for path in sorted(module_dir.rglob("*.rs")))
    return "\n".join(texts)


def find_matching_delimiter(text: str, start: int, opening: str, closing: str) -> int:
    if start >= len(text) or text[start] != opening:
        fail(f"delimiter {opening} not found at expected position")
    depth = 0
    for index in range(start, len(text)):
        char = text[index]
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
    fail(f"unterminated delimiter {opening}{closing}")


def extract_string_literal_prefix(text: str) -> str | None:
    stripped = text.lstrip()
    if not stripped.startswith('"'):
        return None
    escaped = False
    chars: list[str] = []
    for char in stripped[1:]:
        if escaped:
            chars.append(char)
            escaped = False
            continue
        if char == "\\":
            escaped = True
            continue
        if char == '"':
            return "".join(chars)
        chars.append(char)
    return None


def rust_routes() -> set[str]:
    text = rust_source_text(API_SRC)
    routes: set[str] = set()
    offset = 0
    needle = ".route("
    while True:
        start = text.find(needle, offset)
        if start == -1:
            break
        open_paren = start + len(".route")
        close_paren = find_matching_delimiter(text, open_paren, "(", ")")
        literal = extract_string_literal_prefix(text[open_paren + 1 : close_paren])
        if literal is not None:
            routes.add(normalize_path(literal))
        offset = close_paren + 1
    return routes


def rust_handler_body(name: str) -> str:
    text = rust_source_text(API_SRC)
    marker = f"async fn {name}"
    signature_start = text.rfind(marker)
    if signature_start == -1:
        fail(f"handler {name} not found")
    body_start = text.find("{", signature_start)
    if body_start == -1:
        fail(f"handler {name} body start not found")
    body_end = find_matching_delimiter(text, body_start, "{", "}")
    return text[signature_start : body_end + 1]


def import_control_models():
    sys.path.insert(0, str(CONTROL / "src"))
    return importlib.import_module("hermes_polymarket_executor_adapter.models")


def import_control_client():
    sys.path.insert(0, str(CONTROL / "src"))
    return importlib.import_module("hermes_polymarket_executor_adapter.client")


def import_module_from_path(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        fail(f"unable to load module {name} from {path}")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        if previous is None:
            sys.modules.pop(name, None)
        else:
            sys.modules[name] = previous
        raise
    return module


def python_function_body(text: str, name: str) -> str:
    try:
        tree = ast.parse(text)
    except SyntaxError as exc:
        fail(f"python source malformed while locating {name}: {exc}")
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.name == name:
            if getattr(node, "end_lineno", None) is None:
                fail(f"python function {name} missing end line metadata")
            lines = text.splitlines()
            return "\n".join(lines[node.lineno - 1 : node.end_lineno])
    fail(f"python function {name} not found")
