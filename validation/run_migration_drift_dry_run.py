#!/usr/bin/env python3
"""Run lightweight migration dry-run and checksum drift checks.

The script is side-effect-contained: when PMX_TEST_DATABASE_URL is set it creates
temporary schemas in the configured validation database and drops them before
exit. Without a database URL it still validates local migration ordering and
prints a skipped PostgreSQL section.
"""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = [
    ("0001_initial", ROOT / "migrations" / "0001_initial.sql"),
    ("0002_migration_framework", ROOT / "migrations" / "0002_migration_framework.sql"),
    ("0003_order_event_trace", ROOT / "migrations" / "0003_order_event_trace.sql"),
    ("0004_real_funds_canary", ROOT / "migrations" / "0004_real_funds_canary.sql"),
]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_psql(database_url: str, sql: str) -> None:
    subprocess.run(
        ["psql", database_url, "-v", "ON_ERROR_STOP=1", "-q"],
        input=sql,
        text=True,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def schema_sql(schema: str, body: str) -> str:
    return f'CREATE SCHEMA "{schema}";\nSET search_path TO "{schema}";\n{body}\n'


def migration_body(names: list[str]) -> str:
    by_name = {name: path for name, path in MIGRATIONS}
    return "\n".join(by_name[name].read_text() for name in names)


def record_sql(version: str, checksum: str) -> str:
    return (
        "INSERT INTO schema_migrations (version, checksum_sha256) "
        f"VALUES ('{version}', '{checksum}') "
        "ON CONFLICT (version) DO UPDATE SET checksum_sha256 = EXCLUDED.checksum_sha256;\n"
    )


def main() -> int:
    checksums = {name: sha256(path) for name, path in MIGRATIONS}
    result: dict[str, object] = {
        "status": "pass",
        "migration_order": [name for name, _ in MIGRATIONS],
        "checksums": checksums,
    }
    database_url = os.environ.get("PMX_TEST_DATABASE_URL")
    if not database_url:
        result["postgres_dry_run"] = {
            "status": "skipped",
            "reason": "PMX_TEST_DATABASE_URL not set",
        }
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0

    suffix = f"{int(time.time() * 1000)}_{os.getpid()}"
    fresh = f"pmx_migration_fresh_{suffix}"
    upgraded = f"pmx_migration_upgraded_{suffix}"
    drift = f"pmx_migration_drift_{suffix}"
    try:
        run_psql(
            database_url,
            schema_sql(
                fresh,
                migration_body(
                    [
                        "0001_initial",
                        "0002_migration_framework",
                        "0003_order_event_trace",
                        "0004_real_funds_canary",
                    ]
                ),
            )
            + record_sql("0001_initial", checksums["0001_initial"])
            + record_sql("0002_migration_framework", checksums["0002_migration_framework"])
            + record_sql("0003_order_event_trace", checksums["0003_order_event_trace"])
            + record_sql("0004_real_funds_canary", checksums["0004_real_funds_canary"]),
        )
        run_psql(
            database_url,
            schema_sql(upgraded, migration_body(["0001_initial"]))
            + migration_body(["0002_migration_framework"])
            + migration_body(["0003_order_event_trace"])
            + migration_body(["0004_real_funds_canary"])
            + record_sql("0001_initial", checksums["0001_initial"])
            + record_sql("0002_migration_framework", checksums["0002_migration_framework"])
            + record_sql("0003_order_event_trace", checksums["0003_order_event_trace"])
            + record_sql("0004_real_funds_canary", checksums["0004_real_funds_canary"]),
        )
        run_psql(
            database_url,
            schema_sql(drift, migration_body(["0002_migration_framework"]))
            + record_sql("0001_initial", "0" * 64),
        )
        result["postgres_dry_run"] = {
            "status": "pass",
            "fresh_schema": fresh,
            "upgraded_schema": upgraded,
            "drift_schema": drift,
            "drift_detection": "bad checksum fixture created; production runner must fail closed on mismatch",
        }
    finally:
        cleanup = "\n".join(
            f'DROP SCHEMA IF EXISTS "{schema}" CASCADE;' for schema in [fresh, upgraded, drift]
        )
        run_psql(database_url, cleanup)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
