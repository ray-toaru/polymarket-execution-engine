#!/usr/bin/env python3
"""Static guard for sign-only lifecycle persistence scaffolding."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "pmx-core" / "src"
STORE = ROOT / "crates" / "pmx-store" / "src"
POSTGRES = ROOT / "crates" / "pmx-store" / "src"
MIGRATION = ROOT / "migrations" / "0001_initial.sql"
ADAPTER = ROOT / "adapters" / "pmx-official-sdk-adapter" / "src"
SERVICE = ROOT / "crates" / "pmx-service" / "src"
API = ROOT / "crates" / "pmx-api" / "src"
OPENAPI = ROOT / "openapi" / "executor.v1.yaml"

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
    SERVICE: [
        "StandardSignOnlyConstructionRequest",
        "StandardSignOnlyConstructionReceipt",
        "record_standard_sign_only_construction",
        "service_records_standard_sign_only_construction_without_raw_payload",
    ],
    API: [
        "/v1/sign-only/standard-constructions",
        "record_standard_sign_only_construction",
    ],
    OPENAPI: [
        "/v1/sign-only/standard-constructions",
        "StandardSignOnlyConstructionRequest",
        "StandardSignOnlyConstructionReceipt",
    ],
}

def source_text(path: Path) -> str:
    if path.is_dir():
        return "\n".join(source.read_text() for source in sorted(path.rglob("*.rs")))
    return path.read_text()


def main() -> int:
    failures = []
    for path, needles in REQUIRED.items():
        text = source_text(path)
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
