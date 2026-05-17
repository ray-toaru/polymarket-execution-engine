#!/usr/bin/env python3
"""Validate the PostgreSQL migration framework guardrails."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = ROOT / "migrations"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
RUN_GATES = ROOT / "validation" / "run_v0_23_gates.sh"
MANIFEST_WRITER = ROOT / "validation" / "write_v0_23_evidence_manifest.py"


def require(condition: bool, message: str, failures: list[str]) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    failures: list[str] = []
    migration_0002 = MIGRATIONS / "0002_migration_framework.sql"
    postgres = POSTGRES.read_text()
    gates = RUN_GATES.read_text()
    manifest = MANIFEST_WRITER.read_text()

    require(migration_0002.exists(), "missing migrations/0002_migration_framework.sql", failures)
    if migration_0002.exists():
        migration_sql = migration_0002.read_text()
        require("CREATE TABLE IF NOT EXISTS schema_migrations" in migration_sql, "0002 must create schema_migrations", failures)
        require("checksum_sha256 TEXT NOT NULL" in migration_sql, "schema_migrations must store checksum_sha256", failures)
        require("PRIMARY KEY" in migration_sql and "version TEXT" in migration_sql, "schema_migrations must key by version", failures)

    require("SCHEMA_MIGRATIONS" in postgres, "PostgresStore must define ordered SCHEMA_MIGRATIONS", failures)
    require("0001_initial" in postgres, "SCHEMA_MIGRATIONS must include 0001_initial", failures)
    require("0002_migration_framework" in postgres, "SCHEMA_MIGRATIONS must include 0002_migration_framework", failures)
    require("record_applied_migration" in postgres, "apply_schema must record applied migrations", failures)
    require("schema migration checksum mismatch" in postgres, "migration checksum drift must fail closed", failures)
    require("applied_schema_migrations" in postgres, "store must expose applied migration evidence for PG tests", failures)
    require("postgres_records_schema_migrations" in postgres, "PG tests must assert schema_migrations rows", failures)

    require("33-migration-framework-guard.log" in gates, "run_v0_23_gates.sh must emit migration framework guard log", failures)
    require('"migration_framework_validation"' in manifest, "evidence manifest must include migration_framework_validation", failures)
    require("33-migration-framework-guard.log" in manifest, "evidence manifest must capture migration framework guard log", failures)

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("migration framework guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
