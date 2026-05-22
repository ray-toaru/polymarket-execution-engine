#!/usr/bin/env python3
"""Validate the PostgreSQL migration framework guardrails."""
from __future__ import annotations

import sys
from pathlib import Path

from current_gate_chain import require_current_gate_log

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = ROOT / "migrations"
POSTGRES = ROOT / "crates" / "pmx-store" / "src"
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
    migration_0004 = MIGRATIONS / "0004_real_funds_canary.sql"
    migration_0005 = MIGRATIONS / "0005_constraint_decision_snapshot_nullable.sql"
    migration_0006 = MIGRATIONS / "0006_runtime_kill_switch_scope.sql"
    migration_0007 = MIGRATIONS / "0007_runtime_global_kill_switch.sql"
    migration_names = [path.stem for path in sorted(MIGRATIONS.glob("[0-9]*.sql"))]
    postgres = read_rust_sources(POSTGRES)
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
    require("0004_real_funds_canary" in postgres, "SCHEMA_MIGRATIONS must include 0004_real_funds_canary", failures)
    require(
        "0005_constraint_decision_snapshot_nullable" in postgres,
        "SCHEMA_MIGRATIONS must include 0005_constraint_decision_snapshot_nullable",
        failures,
    )
    for migration_name in migration_names:
        require(
            migration_name in postgres,
            f"SCHEMA_MIGRATIONS must include {migration_name}",
            failures,
        )
    require(migration_0003.exists(), "missing migrations/0003_order_event_trace.sql", failures)
    if migration_0003.exists():
        migration_sql = migration_0003.read_text()
        require("ADD COLUMN IF NOT EXISTS correlation_id" in migration_sql, "0003 must add order_events.correlation_id", failures)
        require("idx_order_events_order_correlation" in migration_sql, "0003 must index order event correlation lookup", failures)
    require(migration_0004.exists(), "missing migrations/0004_real_funds_canary.sql", failures)
    if migration_0004.exists():
        migration_sql = migration_0004.read_text()
        require("CREATE TABLE IF NOT EXISTS real_funds_canary_runs" in migration_sql, "0004 must create real_funds_canary_runs", failures)
        require("UNIQUE (account_id, idempotency_key)" in migration_sql, "0004 must enforce canary idempotency", failures)
        require("real_funds_canary_no_raw_signed_order" in migration_sql, "0004 must forbid raw signed order exposure", failures)
    require(
        migration_0005.exists(),
        "missing migrations/0005_constraint_decision_snapshot_nullable.sql",
        failures,
    )
    if migration_0005.exists():
        migration_sql = migration_0005.read_text()
        require(
            "ALTER COLUMN snapshot_id DROP NOT NULL" in migration_sql,
            "0005 must align upgraded constraint_decisions.snapshot_id nullability",
            failures,
        )
    require(migration_0006.exists(), "missing migrations/0006_runtime_kill_switch_scope.sql", failures)
    if migration_0006.exists():
        migration_sql = migration_0006.read_text()
        require("ADD COLUMN IF NOT EXISTS kill_switch_version" in migration_sql, "0006 must add account kill-switch version", failures)
        require("idx_runtime_accounts_kill_switch" in migration_sql, "0006 must index account kill-switch lookup", failures)
    require(migration_0007.exists(), "missing migrations/0007_runtime_global_kill_switch.sql", failures)
    if migration_0007.exists():
        migration_sql = migration_0007.read_text()
        require("CREATE TABLE IF NOT EXISTS runtime_global_controls" in migration_sql, "0007 must create runtime_global_controls", failures)
        require("control_key = 'kill_switch'" in migration_sql, "0007 must key the global kill switch control", failures)
    require("record_applied_migration" in postgres, "apply_schema must record applied migrations", failures)
    require("schema migration checksum mismatch" in postgres, "migration checksum drift must fail closed", failures)
    require("applied_schema_migrations" in postgres, "store must expose applied migration evidence for PG tests", failures)
    require("postgres_records_schema_migrations" in postgres, "PG tests must assert schema_migrations rows", failures)
    require("PMX_TEST_DATABASE_URL" in drift_dry_run, "migration dry-run must support PG validation env", failures)
    require("fresh_schema" in drift_dry_run and "upgraded_schema" in drift_dry_run, "migration dry-run must cover fresh and upgraded schemas", failures)
    require("bad checksum fixture" in drift_dry_run, "migration dry-run must include checksum drift fixture", failures)
    require("glob(\"[0-9]*.sql\")" in drift_dry_run, "migration dry-run must discover all numbered migrations", failures)
    require("migration_names()" in drift_dry_run, "migration dry-run must apply the discovered migration list", failures)
    require("record_all_sql" in drift_dry_run, "migration dry-run must record all discovered checksums", failures)

    require_current_gate_log("33-migration-framework-guard.log", "migration framework guard", failures)
    require_current_gate_log("34-migration-drift-dry-run.log", "migration drift dry-run", failures)
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
