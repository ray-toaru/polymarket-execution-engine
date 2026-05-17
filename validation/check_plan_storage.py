#!/usr/bin/env python3
"""Guard against execution_plans / plan_summaries dual-table drift."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATION = ROOT / "migrations" / "0001_initial.sql"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"


def main() -> int:
    failures: list[str] = []
    migration = MIGRATION.read_text()
    postgres = POSTGRES.read_text()
    if "DROP TABLE IF EXISTS plan_summaries" not in migration:
        failures.append("migration must explicitly remove legacy plan_summaries")
    if "CREATE TABLE IF NOT EXISTS plan_summaries" in migration:
        failures.append("migration must not recreate plan_summaries")
    if "INSERT INTO plan_summaries" in postgres or '"plan_summaries"' in postgres:
        failures.append("PostgresStore must not read/write plan_summaries")
    if "INSERT INTO execution_plans" not in postgres:
        failures.append("PostgresStore must write canonical execution_plans")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("plan storage guard passed: execution_plans is canonical")
    return 0


if __name__ == "__main__":
    sys.exit(main())
