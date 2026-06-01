use super::super::*;

#[tokio::test]
async fn service_records_heartbeat_lease_election_tick_fail_closed_for_non_owner() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    for capability in ["heartbeat", "reconcile", "resource-refresh"] {
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: format!("worker-{capability}"),
                role: "service-test".into(),
                capability: capability.into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record worker heartbeat");
    }

    let observed_at = Utc::now();
    let receipt = record_heartbeat_lease_election_tick(
        &store,
        HeartbeatLeaseElectionTick {
            account_id: "acct-1".into(),
            provider_name: "heartbeat-lease-election-test".into(),
            instance_id: "worker-b".into(),
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            candidates: vec![
                HeartbeatLeaseCandidate {
                    worker_id: "worker-a".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: observed_at - chrono::Duration::seconds(1),
                    last_error: None,
                },
                HeartbeatLeaseCandidate {
                    worker_id: "worker-b".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: observed_at - chrono::Duration::seconds(2),
                    last_error: None,
                },
            ],
        },
    )
    .await
    .expect("record heartbeat lease election tick");
    assert_eq!(receipt.election.lease_owner_id, "worker-a");
    assert!(receipt.election.fail_closed);
    assert!(!receipt.provider_tick.lease_owner_active);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
}
