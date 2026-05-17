#!/usr/bin/env python3
"""Static guard for runtime worker model and store-writer scaffolding."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "crates" / "pmx-runtime" / "src" / "lib.rs"
STORE = ROOT / "crates" / "pmx-store" / "src" / "lib.rs"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
MIGRATION = ROOT / "migrations" / "0001_initial.sql"

REQUIRED = {
    RUNTIME: [
        "pub enum RuntimeWorkerKind",
        "pub struct RuntimeWorkerAction",
        "worker_actions_from_runtime_signals",
        "pub struct RuntimeWorkerStoreWrite",
        "runtime_worker_store_writes",
        "should_fail_closed",
        "should_update_runtime_store",
        "worker_actions_mark_stale_runtime_inputs_as_fail_closed_updates",
        "runtime_worker_store_writes_are_fail_closed_for_bad_signals",
    ],
    STORE: [
        "pub struct RuntimeWorkerObservation",
        "pub trait RuntimeWorkerObservationStore",
        "record_runtime_worker_observation",
    ],
    POSTGRES: [
        "impl RuntimeWorkerObservationStore for PostgresStore",
        "INSERT INTO runtime_worker_observations",
        "postgres_records_runtime_worker_observation",
    ],
    MIGRATION: [
        "CREATE TABLE IF NOT EXISTS runtime_worker_observations",
        "idx_runtime_worker_observations_account_created",
        "idx_runtime_worker_observations_account_capability_observed",
    ],
}

def main() -> int:
    failures = []
    for path, needles in REQUIRED.items():
        text = path.read_text()
        for needle in needles:
            if needle not in text:
                failures.append(f"{path.relative_to(ROOT)} missing {needle}")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("runtime worker model static guard passed")
    return 0

if __name__ == "__main__":
    sys.exit(main())
