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
        "ResourceRefresh",
        "RuntimeWorkerLoopInput",
        "RuntimeWorkerProviderSnapshot",
        "RuntimeWorkerProvider",
        "RuntimeWorkerLoopTick",
        "HeartbeatLeaseCandidate",
        "HeartbeatLeaseElectionInput",
        "HeartbeatLeaseElection",
        "elect_heartbeat_lease_owner",
        "ResourceRefreshObservation",
        "ResourceRefreshEvaluationInput",
        "ResourceRefreshEvaluation",
        "evaluate_resource_refresh_freshness",
        "runtime_worker_loop_tick",
        "runtime_worker_loop_tick_from_provider",
        "should_fail_closed",
        "should_update_runtime_store",
        "worker_actions_mark_stale_runtime_inputs_as_fail_closed_updates",
        "runtime_worker_store_writes_are_fail_closed_for_bad_signals",
        "runtime_worker_loop_tick_blocks_stale_down_and_geoblocked_submit",
        "runtime_worker_loop_tick_recovers_only_after_all_required_inputs_are_healthy",
        "runtime_worker_provider_snapshot_feeds_loop_without_trading_side_effects",
        "heartbeat_lease_election_selects_fresh_owner_and_fails_closed_for_non_owner",
        "heartbeat_lease_election_has_no_owner_when_all_candidates_are_stale",
        "resource_refresh_evaluation_accepts_fresh_healthy_observations",
        "resource_refresh_evaluation_fails_closed_for_stale_failed_or_missing_inputs",
    ],
    ROOT / "crates" / "pmx-service" / "src" / "lib.rs": [
        "record_runtime_worker_signals",
        "record_runtime_worker_tick",
        "record_runtime_worker_provider_snapshot",
        "record_heartbeat_lease_election_tick",
        "record_resource_refresh_worker_tick",
        "RuntimeWorkerTick",
        "RuntimeWorkerTickReceipt",
        "RuntimeWorkerProviderTickReceipt",
        "HeartbeatLeaseElectionTick",
        "HeartbeatLeaseElectionTickReceipt",
        "ResourceRefreshWorkerTick",
        "ResourceRefreshWorkerTickReceipt",
        "runtime_worker_store_writes",
        "RuntimeWorkerObservationStore",
        "service_records_runtime_worker_signals_for_decision_gate",
        "service_records_runtime_worker_tick_heartbeat_and_observations",
        "service_records_runtime_worker_provider_snapshot_for_decision_gate",
        "service_records_heartbeat_lease_election_tick_fail_closed_for_non_owner",
        "service_records_resource_refresh_worker_tick_for_decision_gate",
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
