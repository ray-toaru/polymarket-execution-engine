#!/usr/bin/env python3
"""Validate the PostgreSQL migration framework guardrails."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = ROOT / "migrations"
POSTGRES = ROOT / "crates" / "pmx-store" / "src"
RUN_GATES = ROOT / "validation" / "run_v0_24_gates.sh"
MANIFEST_WRITER = ROOT / "validation" / "write_current_evidence_manifest.py"
DRIFT_DRY_RUN = ROOT / "validation" / "run_migration_drift_dry_run.py"


def require(condition: bool, message: str, failures: list[str]) -> None:
    if not condition:
        failures.append(message)


def read_rust_sources(path: Path) -> str:
    return "\n".join(source.read_text() for source in sorted(path.rglob("*.rs")))


def main() -> int:
    failures: list[str] = []
    migration_0002 = MIGRATIONS / "0002_migration_framework.sql"
    migration_0003 = MIGRATIONS / "0003_order_event_trace.sql"
    postgres = read_rust_sources(POSTGRES)
    gates = RUN_GATES.read_text()
    manifest = MANIFEST_WRITER.read_text()
    drift_dry_run = DRIFT_DRY_RUN.read_text()

    require(migration_0002.exists(), "missing migrations/0002_migration_framework.sql", failures)
    if migration_0002.exists():
        migration_sql = migration_0002.read_text()
        require("CREATE TABLE IF NOT EXISTS schema_migrations" in migration_sql, "0002 must create schema_migrations", failures)
        require("checksum_sha256 TEXT NOT NULL" in migration_sql, "schema_migrations must store checksum_sha256", failures)
        require("PRIMARY KEY" in migration_sql and "version TEXT" in migration_sql, "schema_migrations must key by version", failures)

    require("SCHEMA_MIGRATIONS" in postgres, "PostgresStore must define ordered SCHEMA_MIGRATIONS", failures)
    require("0001_initial" in postgres, "SCHEMA_MIGRATIONS must include 0001_initial", failures)
    require("0002_migration_framework" in postgres, "SCHEMA_MIGRATIONS must include 0002_migration_framework", failures)
    require("0003_order_event_trace" in postgres, "SCHEMA_MIGRATIONS must include 0003_order_event_trace", failures)
    require(migration_0003.exists(), "missing migrations/0003_order_event_trace.sql", failures)
    if migration_0003.exists():
        migration_sql = migration_0003.read_text()
        require("ADD COLUMN IF NOT EXISTS correlation_id" in migration_sql, "0003 must add order_events.correlation_id", failures)
        require("idx_order_events_order_correlation" in migration_sql, "0003 must index order event correlation lookup", failures)
    require("record_applied_migration" in postgres, "apply_schema must record applied migrations", failures)
    require("schema migration checksum mismatch" in postgres, "migration checksum drift must fail closed", failures)
    require("applied_schema_migrations" in postgres, "store must expose applied migration evidence for PG tests", failures)
    require("postgres_records_schema_migrations" in postgres, "PG tests must assert schema_migrations rows", failures)
    require("PMX_TEST_DATABASE_URL" in drift_dry_run, "migration dry-run must support PG validation env", failures)
    require("fresh_schema" in drift_dry_run and "upgraded_schema" in drift_dry_run, "migration dry-run must cover fresh and upgraded schemas", failures)
    require("bad checksum fixture" in drift_dry_run, "migration dry-run must include checksum drift fixture", failures)
    require("0003_order_event_trace" in drift_dry_run, "migration dry-run must include 0003_order_event_trace", failures)

    require("33-migration-framework-guard.log" in gates, "run_v0_24_gates.sh must emit migration framework guard log", failures)
    require("34-migration-drift-dry-run.log" in gates, "run_v0_24_gates.sh must emit migration drift dry-run log", failures)
    require('"migration_framework_validation"' in manifest, "evidence manifest must include migration_framework_validation", failures)
    require("33-migration-framework-guard.log" in manifest, "evidence manifest must capture migration framework guard log", failures)
    require("34-migration-drift-dry-run.log" in manifest, "evidence manifest must capture migration drift dry-run log", failures)

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("migration framework guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
