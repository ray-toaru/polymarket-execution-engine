#!/usr/bin/env python3
"""Run a PostgreSQL-backed real-funds canary CLI preflight without remote side effects."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import time
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ADAPTER_MANIFEST = ROOT / "adapters" / "pmx-official-sdk-adapter" / "Cargo.toml"
CANARY_CLI = ROOT / "adapters" / "pmx-official-sdk-adapter" / "target" / "debug" / "pmx-real-funds-canary"


def load_env_file(path: Path) -> None:
    if not path.exists():
        return
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip("'").strip('"')
        if key and key not in os.environ:
            os.environ[key] = value


def database_url() -> str:
    load_env_file(ROOT / ".env")
    url = os.environ.get("PMX_TEST_DATABASE_URL") or os.environ.get("PMX_DATABASE_URL")
    if not url or not url.strip():
        raise SystemExit("PMX_TEST_DATABASE_URL or PMX_DATABASE_URL is required")
    os.environ["PMX_TEST_DATABASE_URL"] = url
    return url


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
                    "stderr": redact(result.stderr),
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
            "pmx-real-funds-canary",
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
    candidate["book_snapshot_timestamp"] = captured_at
    candidate["active"] = True
    candidate["accepting_orders"] = True
    candidate["closed"] = False
    candidate["archived"] = False
    candidate["side"] = "BUY"
    candidate["order_type"] = "GTC"
    candidate["post_only"] = True
    candidate["target_size"] = "5"
    candidate["exchange_rule_snapshot"] = {
        **candidate.get("exchange_rule_snapshot", {}),
        "schema_version": 1,
        "venue": "polymarket_clob",
        "order_mode": "post_only_limit",
        "order_type": "GTC",
        "side": "BUY",
        "target_size_semantics": "outcome_shares",
        "min_share_size": "5",
        "min_tick_size": "0.01",
        "source": "local-store-truth-cli-preflight",
        "captured_at": captured_at,
        "expires_at": expires_at,
        "evidence_ref": "local://store-truth-cli-preflight/rule-snapshot",
    }
    candidate["human_review_ref"] = "local://store-truth-cli-preflight/human-review"
    return candidate


def approval(account_id: str, market_sha: str) -> dict[str, Any]:
    return {
        "approval_id": "approval-store-truth-cli-preflight",
        "approval_hash": "a" * 64,
        "account_id": account_id,
        "scope": "REAL_FUNDS_CANARY",
        "expires_at": "2099-01-01T00:00:00Z",
        "artifact_sha256": "b" * 64,
        "evidence_manifest_sha256": "c" * 64,
        "workspace_manifest_sha256": "e" * 64,
        "archived_manifest_sha256": "c" * 64,
        "market_candidate_sha256": market_sha,
        "max_order_notional_usd": "1",
        "max_daily_notional_usd": "5",
        "execution_style": "GTC_LIMIT_POST_ONLY_CANCEL",
        "operator_identity_ref": "local-store-truth-cli-preflight",
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
        INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at, updated_at)
        VALUES
          ('store-truth-live-submit-gate-{account_id}', 'CanaryRuntimeTruth', 'live-submit-gate', 'HEALTHY', now(), now()),
          ('store-truth-idempotency-lease-{account_id}', 'CanaryRuntimeTruth', 'idempotency-lease', 'HEALTHY', now(), now()),
          ('store-truth-order-cancel-reconciliation-{account_id}', 'CanaryRuntimeTruth', 'order-cancel-reconciliation', 'HEALTHY', now(), now())
        ON CONFLICT (worker_id) DO UPDATE SET
          role = EXCLUDED.role,
          capability = EXCLUDED.capability,
          status = EXCLUDED.status,
          last_heartbeat_at = EXCLUDED.last_heartbeat_at,
          updated_at = now();
        """,
    )


def run_cli(tmp: Path, account_id: str, condition_id: str) -> dict[str, Any]:
    market = market_candidate()
    market_path = tmp / "candidate-market.json"
    write_json(market_path, market)
    market_sha = hashlib.sha256(market_path.read_bytes()).hexdigest()
    approval_path = tmp / "approval.json"
    write_json(approval_path, approval(account_id, market_sha))
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
    result = subprocess.run(
        [
            str(CANARY_CLI),
            "--preflight-only",
            "--allow-live-submit-config",
            "--allow-real-funds-canary-config",
            "--approval-file",
            str(approval_path),
            "--market-file",
            str(market_path),
            "--artifact-sha256",
            "b" * 64,
            "--evidence-manifest-sha256",
            "c" * 64,
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


def main() -> int:
    url = database_url()
    build_cli()
    suffix = str(time.time_ns())
    account_id = f"acct-store-truth-{suffix}"
    condition_id = f"cond-store-truth-{suffix}"
    seed_runtime_truth(url, account_id, condition_id)
    with tempfile.TemporaryDirectory(prefix="pmx-store-truth-cli-") as tmp_dir:
        report = run_cli(Path(tmp_dir), account_id, condition_id)
    failures: list[str] = []
    if report.get("status") != "preflight_ready":
        failures.append("CLI did not report preflight_ready")
    for key, expected in [
        ("posted", False),
        ("remote_side_effects", False),
        ("raw_signed_order_exposed", False),
        ("live_submit_allowed", True),
        ("real_funds_canary_allowed", True),
    ]:
        if report.get(key) is not expected:
            failures.append(f"unexpected {key}: {report.get(key)!r}")
    result = {
        "status": "fail" if failures else "pass",
        "preflight_ready": report.get("status") == "preflight_ready",
        "runtime_truth_source": "postgres",
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
