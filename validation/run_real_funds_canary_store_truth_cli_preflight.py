#!/usr/bin/env python3
"""Run a PostgreSQL-backed real-funds canary CLI preflight without remote side effects."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
import time
import tomllib
from datetime import UTC, datetime, timedelta
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
ADAPTER_MANIFEST = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.toml"
CANARY_CLI = (
    ROOT / "adapters" / "pmx-official-sdk-adapter" / "target" / "debug" / "pmx-real-funds-canary-preflight"
)
ARTIFACT_SHA256 = "b" * 64
EVIDENCE_MANIFEST_SHA256 = "c" * 64
WORKSPACE_MANIFEST_SHA256 = "e" * 64
SYNTHETIC_ACTIVE_PROFILE = "store_truth_cli_preflight"
ENV_REFERENCE_PATTERN = re.compile(r"\$\{([A-Z0-9_]+)\}")
PREFLIGHT_GATE_EVIDENCE_PATHS = {
    "live_submit_allowed": "worker_health/live-submit-gate",
    "real_funds_canary_allowed": "authorizations/real-funds-canary",
    "preconditions_live_submit_would_pass": "preflight/live-submit-preconditions",
    "preconditions_real_funds_canary_would_pass": "preflight/real-funds-canary-preconditions",
    "kill_switch_open": "runtime_accounts/kill-switch",
    "runtime_worker_healthy": "worker_health/runtime-worker",
    "geoblock_allowed": "compliance/geoblock",
    "repository_reservation_exists": "repository/reservation",
    "idempotency_key_written": "worker_health/idempotency-lease",
    "reconcile_worker_healthy": "worker_health/reconcile-worker",
    "cancel_only_fallback_ready": "operations/cancel-only-fallback",
    "balance_allowance_checked": "balances/allowance-check",
}


def is_sha256(value: str) -> bool:
    return len(value) == 64 and all(ch in "0123456789abcdefABCDEF" for ch in value)


def require_sha256(value: str, field: str) -> str:
    if not is_sha256(value):
        raise SystemExit(f"{field} must be 64-hex")
    return value.lower()


def resolve_env_references(value: str, known: dict[str, str]) -> str:
    resolved = value
    for _ in range(8):
        updated = ENV_REFERENCE_PATTERN.sub(
            lambda match: known.get(match.group(1), os.environ.get(match.group(1), match.group(0))),
            resolved,
        )
        if updated == resolved:
            break
        resolved = updated
    return resolved


def load_env_file(path: Path) -> None:
    if not path.exists():
        return
    loaded: dict[str, str] = {}
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = resolve_env_references(value.strip().strip("'").strip('"'), loaded)
        if key and key not in os.environ:
            os.environ[key] = value
        loaded[key] = os.environ.get(key, value)


def load_default_env_files(*, runtime_secrets_env_file: Path | None = None) -> None:
    if runtime_secrets_env_file is not None:
        load_env_file(runtime_secrets_env_file)


def database_url(*, runtime_secrets_env_file: Path | None = None) -> str:
    load_default_env_files(runtime_secrets_env_file=runtime_secrets_env_file)
    url = os.environ.get("PMX_TEST_DATABASE_URL") or os.environ.get("PMX_DATABASE_URL")
    if not url or not url.strip():
        raise SystemExit("PMX_TEST_DATABASE_URL or PMX_DATABASE_URL is required")
    os.environ["PMX_TEST_DATABASE_URL"] = url
    return url


def database_target_summary(url: str) -> dict[str, Any]:
    parsed = urlparse(url)
    default_port = 5432 if parsed.scheme.startswith("postgres") else None
    return {
        "scheme": parsed.scheme or "unknown",
        "hostname": parsed.hostname or "unknown",
        "port": parsed.port or default_port,
        "database": parsed.path.lstrip("/") or "unknown",
        "username": parsed.username or "<none>",
    }


def trimmed_output(text: str) -> str:
    stripped = redact(text).strip()
    return stripped or "<empty>"


def check_database_connectivity(url: str) -> None:
    env = os.environ.copy()
    env["PGCONNECT_TIMEOUT"] = "5"
    result = subprocess.run(
        ["psql", url, "-v", "ON_ERROR_STOP=1", "-qAt", "-c", "select 1;"],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            json.dumps(
                {
                    "status": "fail",
                    "stage": "database_connectivity_preflight",
                    "database_target": database_target_summary(url),
                    "returncode": result.returncode,
                    "stdout": trimmed_output(result.stdout),
                    "stderr": trimmed_output(result.stderr),
                },
                indent=2,
                sort_keys=True,
            )
        )


def run_psql(url: str, sql: str) -> None:
    env = os.environ.copy()
    env["PGCONNECT_TIMEOUT"] = "5"
    result = subprocess.run(
        ["psql", url, "-v", "ON_ERROR_STOP=1", "-qAt", "-c", sql],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            json.dumps(
                {
                    "status": "fail",
                    "stage": "seed_postgres_runtime_truth",
                    "database_target": database_target_summary(url),
                    "returncode": result.returncode,
                    "stdout": trimmed_output(result.stdout),
                    "stderr": trimmed_output(result.stderr),
                },
                indent=2,
                sort_keys=True,
            )
        )


def build_cli() -> None:
    result = subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(ADAPTER_MANIFEST),
            "--features",
            "live-submit",
            "--locked",
            "--bin",
            "pmx-real-funds-canary-preflight",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            json.dumps(
                {
                    "status": "fail",
                    "stage": "build_canary_cli",
                    "stderr": redact(result.stderr),
                    "stdout": redact(result.stdout),
                },
                indent=2,
                sort_keys=True,
            )
        )


def redact(text: str) -> str:
    for key in ["PMX_TEST_DATABASE_URL", "PMX_DATABASE_URL"]:
        value = os.environ.get(key)
        if value:
            text = text.replace(value, "<redacted-db-url>")
    return text


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def with_synthetic_active_profile_env(env: dict[str, str], account_id: str) -> dict[str, str]:
    updated = dict(env)
    updated["PMX_ACTIVE_ACCOUNT_PROFILE"] = SYNTHETIC_ACTIVE_PROFILE
    updated["PMX_ACTIVE_ACCOUNT_ID"] = account_id
    updated["PMX_ACTIVE_PROFILE_REF"] = f"local-profile://{SYNTHETIC_ACTIVE_PROFILE}"
    # Preflight-only execution validates active profile completeness without
    # signing or submitting. Override these names so ambient real credentials
    # are never inherited by the local store-truth gate.
    updated["POLYMARKET_PRIVATE_KEY"] = "synthetic-preflight-private-key-not-used"
    updated["POLY_API_KEY"] = "123e4567-e89b-12d3-a456-426614174000"
    updated["POLY_API_SECRET"] = "synthetic-preflight-api-secret-not-used"
    updated["POLY_API_PASSPHRASE"] = "synthetic-preflight-passphrase-not-used"
    updated["PMX_CLOB_SIGNATURE_TYPE"] = "EOA"
    updated["PMX_CLOB_FUNDER"] = "0x0000000000000000000000000000000000000000"
    return updated


def market_candidate() -> dict[str, Any]:
    candidate_file = os.environ.get("PMX_STORE_TRUTH_CANDIDATE_MARKET_FILE") or os.environ.get(
        "PMX_CANARY_MARKET_FILE"
    )
    if candidate_file:
        candidate = json.loads(Path(candidate_file).read_text())
    else:
        candidate = {
            "market_id": "0xb0a9e9c70cd5bff7feb2b7038ff7e37412b07a8bcfc2e4aff1568aff77641cc4",
            "token_id": "76257837601510063190648803674187298966745324898157392917675508898085965971320",
            "side": "BUY",
            "order_type": "GTC",
            "post_only": True,
            "active": True,
            "accepting_orders": True,
            "closed": False,
            "archived": False,
            "best_ask": "0.024",
            "limit_price": "0.02",
            "ask_size": "100",
            "target_size": "5",
            "estimated_order_notional_usd": "0.1",
            "spread_bps": 10,
            "min_order_size": "5",
            "exchange_rule_snapshot": {
                "schema_version": 1,
                "venue": "polymarket_clob",
                "order_mode": "post_only_limit",
                "order_type": "GTC",
                "side": "BUY",
                "target_size_semantics": "outcome_shares",
                "min_share_size": "5",
                "min_tick_size": "0.01",
                "source": "local-store-truth-cli-preflight",
                "captured_at": "2099-01-01T00:00:00Z",
                "expires_at": "2099-01-01T00:15:00Z",
                "evidence_ref": "local://store-truth-cli-preflight/rule-snapshot",
            },
            "liquidity_score": 500,
            "book_snapshot_timestamp": "2099-01-01T00:00:00Z",
            "human_review_ref": "local://store-truth-cli-preflight/human-review",
        }
    now = datetime.now(UTC)
    captured_at = now.isoformat().replace("+00:00", "Z")
    expires_at = (now + timedelta(minutes=15)).isoformat().replace("+00:00", "Z")
    candidate.pop("outcome", None)
    candidate["book_snapshot_timestamp"] = captured_at
    candidate["active"] = True
    candidate["accepting_orders"] = True
    candidate["closed"] = False
    candidate["archived"] = False
    candidate["side"] = "BUY"
    candidate["order_type"] = "GTC"
    candidate["post_only"] = True
    candidate["target_size"] = "5"
    candidate["estimated_order_notional_usd"] = estimated_notional(
        candidate.get("limit_price", "0.02"),
        candidate["target_size"],
    )
    existing_snapshot = candidate.get("exchange_rule_snapshot", {})
    if not isinstance(existing_snapshot, dict):
        existing_snapshot = {}
    candidate["exchange_rule_snapshot"] = {
        **existing_snapshot,
        "schema_version": 1,
        "venue": "polymarket_clob",
        "order_mode": "post_only_limit",
        "order_type": "GTC",
        "side": "BUY",
        "target_size_semantics": "outcome_shares",
        "min_share_size": existing_snapshot.get("min_share_size", "5"),
        "min_tick_size": existing_snapshot.get("min_tick_size", "0.01"),
        "source": existing_snapshot.get("source", "local-store-truth-cli-preflight"),
        "captured_at": captured_at,
        "expires_at": expires_at,
        "evidence_ref": existing_snapshot.get("evidence_ref", "local://store-truth-cli-preflight/rule-snapshot"),
    }
    candidate["human_review_ref"] = "local://store-truth-cli-preflight/human-review"
    return candidate


def estimated_notional(limit_price: object, target_size: object) -> str:
    try:
        notional = Decimal(str(limit_price)) * Decimal(str(target_size))
    except (InvalidOperation, ValueError) as exc:
        raise SystemExit("candidate limit_price and target_size must be decimals") from exc
    text = format(notional.normalize(), "f")
    return "0" if text == "-0" else text


def condition_id_from_candidate(candidate: dict[str, Any]) -> str:
    value = str(candidate.get("market_id") or "").strip()
    if not value:
        raise SystemExit("candidate market_id is required for runtime truth condition binding")
    return value


def approval(
    account_id: str,
    condition_id: str,
    market_sha: str,
    *,
    artifact_sha256: str,
    workspace_manifest_sha256: str,
    archived_manifest_sha256: str,
) -> dict[str, Any]:
    operator_identity_ref = "local-store-truth-cli-preflight"
    return {
        "approval_id": "approval-store-truth-cli-preflight",
        "approval_hash": "a" * 64,
        "account_id": account_id,
        "condition_id": condition_id,
        "scope": "REAL_FUNDS_CANARY",
        "expires_at": "2099-01-01T00:00:00Z",
        "artifact_sha256": artifact_sha256,
        "evidence_manifest_sha256": archived_manifest_sha256,
        "workspace_manifest_sha256": workspace_manifest_sha256,
        "archived_manifest_sha256": archived_manifest_sha256,
        "market_candidate_sha256": market_sha,
        "max_order_notional_usd": "1",
        "max_daily_notional_usd": "1",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "operator_identity_ref": operator_identity_ref,
        "operator_identity_sha256": hashlib.sha256(operator_identity_ref.encode()).hexdigest(),
        "runtime_gate_snapshot": {
            "live_submit_allowed": True,
            "real_funds_canary_allowed": True,
            "preconditions_live_submit_would_pass": True,
            "preconditions_real_funds_canary_would_pass": True,
            "kill_switch_open": True,
            "runtime_worker_healthy": True,
            "geoblock_allowed": True,
            "repository_reservation_exists": True,
            "idempotency_key_written": True,
            "reconcile_worker_healthy": True,
            "cancel_only_fallback_ready": True,
            "balance_allowance_checked": True,
        },
        "runtime_gate_evidence_refs": {
            "live_submit_allowed": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/runtime/live-submit-allowed",
            "real_funds_canary_allowed": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/runtime/real-funds-canary-allowed",
            "kill_switch_open": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/runtime_accounts/kill-switch",
            "runtime_worker_healthy": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/worker_health/runtime-worker",
            "geoblock_allowed": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/compliance/geoblock",
            "repository_reservation_exists": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/repository/reservation",
            "idempotency_key_written": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/worker_health/idempotency-lease",
            "reconcile_worker_healthy": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/worker_health/reconcile-worker",
            "cancel_only_fallback_ready": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/operations/cancel-only-fallback",
            "balance_allowance_checked": f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}/balances/allowance-check",
        },
    }


def seed_runtime_truth(url: str, account_id: str, condition_id: str) -> None:
    run_psql(
        url,
        f"""
        INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled)
        VALUES ('{account_id}', 'ACTIVE', false)
        ON CONFLICT (account_id) DO UPDATE SET
          status = EXCLUDED.status,
          kill_switch_enabled = EXCLUDED.kill_switch_enabled,
          updated_at = now();
        INSERT INTO runtime_markets (condition_id, status, is_sports)
        VALUES ('{condition_id}', 'ACTIVE', false)
        ON CONFLICT (condition_id) DO UPDATE SET
          status = EXCLUDED.status,
          is_sports = EXCLUDED.is_sports,
          updated_at = now();
        INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at, updated_at, account_id, condition_id)
        VALUES
          ('store-truth-heartbeat-{account_id}', 'CanaryRuntimeTruth', 'heartbeat', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-reconcile-{account_id}', 'CanaryRuntimeTruth', 'reconcile', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-resource-refresh-{account_id}', 'CanaryRuntimeTruth', 'resource-refresh', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-live-submit-gate-{account_id}', 'CanaryRuntimeTruth', 'live-submit-gate', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-idempotency-lease-{account_id}', 'CanaryRuntimeTruth', 'idempotency-lease', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-order-cancel-reconciliation-{account_id}', 'CanaryRuntimeTruth', 'order-cancel-reconciliation', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-repository-reservation-{account_id}', 'CanaryRuntimeTruth', 'repository-reservation', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-reconcile-worker-{account_id}', 'CanaryRuntimeTruth', 'reconcile-worker', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-cancel-only-fallback-{account_id}', 'CanaryRuntimeTruth', 'cancel-only-fallback', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}'),
          ('store-truth-balance-allowance-check-{account_id}', 'CanaryRuntimeTruth', 'balance-allowance-check', 'HEALTHY', now(), now(), '{account_id}', '{condition_id}')
        ON CONFLICT (worker_id) DO UPDATE SET
          role = EXCLUDED.role,
          capability = EXCLUDED.capability,
          status = EXCLUDED.status,
          last_heartbeat_at = EXCLUDED.last_heartbeat_at,
          account_id = EXCLUDED.account_id,
          condition_id = EXCLUDED.condition_id,
          updated_at = now();
        """,
    )


def run_cli(
    tmp: Path,
    account_id: str,
    condition_id: str,
    *,
    artifact_sha256: str,
    workspace_manifest_sha256: str,
    archived_manifest_sha256: str,
) -> dict[str, Any]:
    market = market_candidate()
    market_path = tmp / "candidate-market.json"
    write_json(market_path, market)
    market_sha = hashlib.sha256(market_path.read_bytes()).hexdigest()
    approval_path = tmp / "approval.json"
    write_json(
        approval_path,
        approval(
            account_id,
            condition_id,
            market_sha,
            artifact_sha256=artifact_sha256,
            workspace_manifest_sha256=workspace_manifest_sha256,
            archived_manifest_sha256=archived_manifest_sha256,
        ),
    )
    env = os.environ.copy()
    env.update(
        {
            "PMX_ALLOW_LIVE_SUBMIT": "1",
            "PMX_ALLOW_REAL_FUNDS_CANARY": "1",
            "PMX_KILL_SWITCH_OPEN": "1",
            "PMX_RUNTIME_WORKER_HEALTHY": "1",
            "PMX_GEOBLOCK_ALLOWED": "1",
            "PMX_REPOSITORY_RESERVATION_EXISTS": "1",
            "PMX_IDEMPOTENCY_KEY_WRITTEN": "1",
            "PMX_RECONCILE_WORKER_HEALTHY": "1",
            "PMX_CANCEL_ONLY_FALLBACK_READY": "1",
            "PMX_BALANCE_ALLOWANCE_CHECKED": "1",
        }
    )
    env = with_synthetic_active_profile_env(env, account_id)
    result = subprocess.run(
        [
            str(CANARY_CLI),
            "--approval-file",
            str(approval_path),
            "--market-file",
            str(market_path),
            "--preflight-only",
            "--allow-live-submit-config",
            "--allow-real-funds-canary-config",
            "--artifact-sha256",
            artifact_sha256,
            "--evidence-manifest-sha256",
            archived_manifest_sha256,
            "--idempotency-key",
            f"idem-store-truth-{account_id}",
            "--account-id",
            account_id,
            "--execution-id",
            f"exec-store-truth-{account_id}",
            "--plan-hash",
            "f" * 64,
            "--daily-used-notional-usd",
            "0",
            "--runtime-truth-store",
            "postgres",
            "--runtime-truth-database-url-env",
            "PMX_TEST_DATABASE_URL",
            "--runtime-truth-condition-id",
            condition_id,
        ],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            json.dumps(
                {
                    "status": "fail",
                    "stage": "run_cli_preflight",
                    "stderr": redact(result.stderr),
                    "stdout": redact(result.stdout),
                },
                indent=2,
                sort_keys=True,
            )
        )
    return json.loads(result.stdout)


def runtime_truth_document(
    account_id: str,
    condition_id: str,
    report: dict[str, Any],
    *,
    artifact_sha256: str = ARTIFACT_SHA256,
    workspace_manifest_sha256: str = WORKSPACE_MANIFEST_SHA256,
    archived_manifest_sha256: str = EVIDENCE_MANIFEST_SHA256,
) -> dict[str, Any]:
    evidence_prefix = f"pg://canary-runtime-truth/account/{account_id}/condition/{condition_id}"
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    gate_evidence_refs = {
        field: f"{evidence_prefix}/{suffix}"
        for field, suffix in PREFLIGHT_GATE_EVIDENCE_PATHS.items()
    }
    preflight_defaults = {
        "posted": False,
        "remote_side_effects": False,
        "raw_signed_order_exposed": False,
        "live_submit_allowed": False,
        "real_funds_canary_allowed": False,
        "preconditions_live_submit_would_pass": True,
        "preconditions_real_funds_canary_would_pass": True,
        "kill_switch_open": True,
        "runtime_worker_healthy": True,
        "geoblock_allowed": True,
        "repository_reservation_exists": True,
        "idempotency_key_written": True,
        "reconcile_worker_healthy": True,
        "cancel_only_fallback_ready": True,
        "balance_allowance_checked": True,
    }

    def report_bool(field: str) -> bool:
        value = report.get(field)
        return value if isinstance(value, bool) else preflight_defaults[field]

    return {
        "schema_version": 1,
        "status": "reviewed_runtime_truth_candidate",
        "source_release": f"v{cargo['workspace']['package']['version']}",
        "scope": "REAL_FUNDS_CANARY",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "account_id": account_id,
        "condition_id": condition_id,
        "artifact_sha256": artifact_sha256,
        "workspace_manifest_sha256": workspace_manifest_sha256,
        "archived_manifest_sha256": archived_manifest_sha256,
        "dependencies": [
            {
                "name": "kill_switch",
                "status": "durable_runtime_truth",
                "evidence_ref": f"{evidence_prefix}/runtime_accounts",
            },
            {
                "name": "live_submit_gate",
                "status": "durable_runtime_truth",
                "evidence_ref": f"{evidence_prefix}/worker_health/live-submit-gate",
            },
            {
                "name": "idempotency_lease",
                "status": "durable_runtime_truth",
                "evidence_ref": f"{evidence_prefix}/worker_health/idempotency-lease",
            },
            {
                "name": "order_cancel_reconciliation",
                "status": "durable_runtime_truth",
                "evidence_ref": f"{evidence_prefix}/worker_health/order-cancel-reconciliation",
            },
        ],
        "references_only_no_secret_values": True,
        "live_submit_allowed": False,
        "live_cancel_allowed": False,
        "real_funds_canary_authorized": False,
        "remote_side_effects": False,
        "production_ready_claimed": False,
        "preflight_report": {
            "status": report.get("status"),
            "runtime_truth_source": "postgres",
            "posted": report_bool("posted"),
            "remote_side_effects": report_bool("remote_side_effects"),
            "raw_signed_order_exposed": report_bool("raw_signed_order_exposed"),
            "live_submit_allowed": report_bool("live_submit_allowed"),
            "real_funds_canary_allowed": report_bool("real_funds_canary_allowed"),
            "preconditions_live_submit_would_pass": report_bool("preconditions_live_submit_would_pass"),
            "preconditions_real_funds_canary_would_pass": report_bool("preconditions_real_funds_canary_would_pass"),
            "kill_switch_open": report_bool("kill_switch_open"),
            "runtime_worker_healthy": report_bool("runtime_worker_healthy"),
            "geoblock_allowed": report_bool("geoblock_allowed"),
            "repository_reservation_exists": report_bool("repository_reservation_exists"),
            "idempotency_key_written": report_bool("idempotency_key_written"),
            "reconcile_worker_healthy": report_bool("reconcile_worker_healthy"),
            "cancel_only_fallback_ready": report_bool("cancel_only_fallback_ready"),
            "balance_allowance_checked": report_bool("balance_allowance_checked"),
            "gate_evidence_refs": gate_evidence_refs,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--runtime-truth-output",
        type=Path,
        help="Optional path for a references-only runtime-truth JSON candidate produced from the seeded PostgreSQL rows.",
    )
    parser.add_argument(
        "--artifact-sha256",
        default=ARTIFACT_SHA256,
        help="Release artifact SHA-256 to bind into the local approval and optional runtime-truth output.",
    )
    parser.add_argument(
        "--workspace-manifest-sha256",
        default=WORKSPACE_MANIFEST_SHA256,
        help="Workspace evidence manifest SHA-256 to bind into the local approval and optional runtime-truth output.",
    )
    parser.add_argument(
        "--archived-manifest-sha256",
        default=EVIDENCE_MANIFEST_SHA256,
        help="Archived release evidence manifest SHA-256 to bind into the local approval and optional runtime-truth output.",
    )
    parser.add_argument(
        "--runtime-secrets-env-file",
        type=Path,
        help="Optional explicit runtime companion secrets env file. The default path is not auto-loaded.",
    )
    args = parser.parse_args()
    artifact_sha256 = require_sha256(args.artifact_sha256, "--artifact-sha256")
    workspace_manifest_sha256 = require_sha256(args.workspace_manifest_sha256, "--workspace-manifest-sha256")
    archived_manifest_sha256 = require_sha256(args.archived_manifest_sha256, "--archived-manifest-sha256")
    runtime_secrets_env_file = None
    if args.runtime_secrets_env_file is not None:
        runtime_secrets_env_file = (
            args.runtime_secrets_env_file
            if args.runtime_secrets_env_file.is_absolute()
            else ROOT / args.runtime_secrets_env_file
        )
    url = database_url(runtime_secrets_env_file=runtime_secrets_env_file)
    check_database_connectivity(url)
    build_cli()
    suffix = str(time.time_ns())
    account_id = os.environ.get("PMX_ACTIVE_ACCOUNT_ID") or f"acct-store-truth-{suffix}"
    condition_id = condition_id_from_candidate(market_candidate())
    seed_runtime_truth(url, account_id, condition_id)
    with tempfile.TemporaryDirectory(prefix="pmx-store-truth-cli-") as tmp_dir:
        report = run_cli(
            Path(tmp_dir),
            account_id,
            condition_id,
            artifact_sha256=artifact_sha256,
            workspace_manifest_sha256=workspace_manifest_sha256,
            archived_manifest_sha256=archived_manifest_sha256,
        )
    failures: list[str] = []
    if report.get("status") != "preflight_ready":
        failures.append("CLI did not report preflight_ready")
    for key, expected in [
        ("posted", False),
        ("remote_side_effects", False),
        ("raw_signed_order_exposed", False),
        ("live_submit_allowed", False),
        ("real_funds_canary_allowed", False),
        ("preconditions_live_submit_would_pass", True),
        ("preconditions_real_funds_canary_would_pass", True),
    ]:
        if report.get(key) is not expected:
            failures.append(f"unexpected {key}: {report.get(key)!r}")
    runtime_truth_path = None
    runtime_truth_sha256 = None
    if not failures and args.runtime_truth_output:
        runtime_truth = runtime_truth_document(
            account_id,
            condition_id,
            report,
            artifact_sha256=artifact_sha256,
            workspace_manifest_sha256=workspace_manifest_sha256,
            archived_manifest_sha256=archived_manifest_sha256,
        )
        args.runtime_truth_output.parent.mkdir(parents=True, exist_ok=True)
        write_json(args.runtime_truth_output, runtime_truth)
        runtime_truth_path = str(args.runtime_truth_output)
        runtime_truth_sha256 = hashlib.sha256(args.runtime_truth_output.read_bytes()).hexdigest()
    result = {
        "status": "fail" if failures else "pass",
        "preflight_ready": report.get("status") == "preflight_ready",
        "runtime_truth_source": "postgres",
        "runtime_truth_output": runtime_truth_path,
        "runtime_truth_output_sha256": runtime_truth_sha256,
        "posted": report.get("posted"),
        "remote_side_effects": report.get("remote_side_effects"),
        "raw_signed_order_exposed": report.get("raw_signed_order_exposed"),
        "selected_market_id_hash_present": bool(report.get("selected_market_id_hash")),
        "selected_token_id_hash_present": bool(report.get("selected_token_id_hash")),
        "failures": failures,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
