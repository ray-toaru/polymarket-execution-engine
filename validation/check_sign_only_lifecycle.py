#!/usr/bin/env python3
"""Static guard for sign-only lifecycle persistence scaffolding."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "pmx-core" / "src" / "lib.rs"
STORE = ROOT / "crates" / "pmx-store" / "src" / "lib.rs"
POSTGRES = ROOT / "crates" / "pmx-store" / "src" / "postgres.rs"
MIGRATION = ROOT / "migrations" / "0001_initial.sql"
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src" / "lib.rs"

REQUIRED = {
    CORE: [
        "pub enum SignOnlyLifecycleState",
        "pub enum SignOnlyLifecycleEventKind",
        "pub struct SignOnlyLifecycleRecord",
        "transition_sign_only_lifecycle",
        "sign_only_lifecycle_has_remote_side_effect",
        "sign_only_lifecycle_never_models_remote_post",
    ],
    STORE: [
        "pub trait SignOnlyLifecycleStore",
        "record_sign_only_lifecycle_event",
        "list_sign_only_lifecycle_events",
        "sign_only_lifecycle_events",
    ],
    POSTGRES: [
        "impl SignOnlyLifecycleStore for PostgresStore",
        "INSERT INTO sign_only_lifecycle_events",
        "postgres_persists_sign_only_lifecycle_records",
    ],
    MIGRATION: [
        "CREATE TABLE IF NOT EXISTS sign_only_lifecycle_events",
        "CHECK (no_remote_side_effect = TRUE)",
    ],
    ADAPTER: [
        "sign_only_lifecycle_records_from_receipt",
        "sign-only receipt unexpectedly indicates remote posting",
        "sign_only_lifecycle_records_are_persistable_and_non_mutating",
        "sign_only_lifecycle_rejects_posted_receipt",
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
    print("sign-only lifecycle static guard passed")
    return 0

if __name__ == "__main__":
    sys.exit(main())
