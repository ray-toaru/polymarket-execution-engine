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
        "ReconcileBacklogEvaluationInput",
        "ReconcileBacklogEvaluation",
        "evaluate_reconcile_backlog",
        "WebSocketLivenessObservation",
        "WebSocketLivenessEvaluationInput",
        "WebSocketLivenessEvaluation",
        "evaluate_websocket_liveness",
        "GeoblockEvaluationInput",
        "GeoblockEvaluation",
        "evaluate_geoblock_status",
        "WorkerCrashRecoveryObservation",
        "WorkerCrashRecoveryEvaluationInput",
        "WorkerCrashRecoveryEvaluation",
        "evaluate_worker_crash_recovery",
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
        "reconcile_backlog_evaluation_blocks_submit_for_remote_unknown_orders",
        "reconcile_backlog_evaluation_allows_submit_when_backlog_empty",
        "websocket_liveness_evaluation_accepts_fresh_market_and_user_channels",
        "websocket_liveness_evaluation_fails_closed_for_disconnected_stale_or_missing_channels",
        "geoblock_evaluation_only_allows_explicit_allowed_status",
        "worker_crash_recovery_evaluation_requires_fresh_healthy_required_workers",
        "worker_crash_recovery_evaluation_recovers_after_all_required_workers_are_fresh",
    ],
    ROOT / "crates" / "pmx-service" / "src" / "lib.rs": [
        "record_runtime_worker_signals",
        "record_runtime_worker_tick",
        "record_runtime_worker_provider_snapshot",
        "record_runtime_worker_continuous_tick",
        "record_heartbeat_lease_election_tick",
        "record_resource_refresh_worker_tick",
        "record_reconcile_backlog_worker_tick",
        "record_websocket_liveness_worker_tick",
        "record_geoblock_worker_tick",
        "record_worker_crash_recovery_tick",
        "RuntimeWorkerTick",
        "RuntimeWorkerTickReceipt",
        "RuntimeWorkerProviderTickReceipt",
        "RuntimeWorkerContinuousTick",
        "RuntimeWorkerContinuousTickReceipt",
        "RuntimeWorkerStatusQuery",
        "RuntimeWorkerStatusReport",
        "list_runtime_worker_status",
        "HeartbeatLeaseElectionTick",
        "HeartbeatLeaseElectionTickReceipt",
        "ResourceRefreshWorkerTick",
        "ResourceRefreshWorkerTickReceipt",
        "ReconcileBacklogWorkerTick",
        "ReconcileBacklogWorkerTickReceipt",
        "WebSocketLivenessWorkerTick",
        "WebSocketLivenessWorkerTickReceipt",
        "GeoblockWorkerTick",
        "GeoblockWorkerTickReceipt",
        "WorkerCrashRecoveryTick",
        "WorkerCrashRecoveryTickReceipt",
        "runtime_worker_store_writes",
        "RuntimeWorkerObservationStore",
        "RuntimeWorkerStatusStore",
        "service_records_runtime_worker_signals_for_decision_gate",
        "service_records_runtime_worker_tick_heartbeat_and_observations",
        "service_lists_runtime_worker_status",
        "service_records_runtime_worker_provider_snapshot_for_decision_gate",
        "service_records_continuous_runtime_worker_ticks_fail_closed_on_any_bad_snapshot",
        "service_records_heartbeat_lease_election_tick_fail_closed_for_non_owner",
        "service_records_resource_refresh_worker_tick_for_decision_gate",
        "service_records_reconcile_backlog_worker_tick_for_decision_gate",
        "service_records_websocket_liveness_worker_tick_for_decision_gate",
        "service_records_geoblock_worker_tick_for_decision_gate",
        "service_records_worker_crash_recovery_tick_for_decision_gate",
    ],
    STORE: [
        "pub struct RuntimeWorkerObservation",
        "pub trait RuntimeWorkerObservationStore",
        "pub trait RuntimeWorkerStatusStore",
        "record_runtime_worker_observation",
        "list_runtime_worker_status",
    ],
    POSTGRES: [
        "impl RuntimeWorkerObservationStore for PostgresStore",
        "impl RuntimeWorkerStatusStore for PostgresStore",
        "INSERT INTO runtime_worker_observations",
        "postgres_records_runtime_worker_observation",
        "postgres_lists_runtime_worker_status",
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
