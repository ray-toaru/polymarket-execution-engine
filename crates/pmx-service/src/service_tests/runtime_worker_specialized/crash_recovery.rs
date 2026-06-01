use super::super::*;

#[tokio::test]
async fn service_records_worker_crash_recovery_tick_for_decision_gate() {
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
    let receipt = record_worker_crash_recovery_tick(
        &store,
        WorkerCrashRecoveryTick {
            account_id: "acct-1".into(),
            worker_id: "worker-crash-recovery".into(),
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::WorkerCrashRecoveryObservation {
                    worker_id: "worker-heartbeat".into(),
                    capability: "heartbeat".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                    last_error: None,
                },
                pmx_runtime::WorkerCrashRecoveryObservation {
                    worker_id: "worker-reconcile".into(),
                    capability: "reconcile".into(),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(60)),
                    last_error: Some("stale after crash".into()),
                },
            ],
        },
    )
    .await
    .expect("record worker crash recovery tick");
    assert!(receipt.heartbeat_recorded);
    assert!(receipt.observation_recorded);
    assert!(!receipt.evaluation.recovered);
    assert_eq!(receipt.evaluation.stale_workers, vec!["worker-reconcile"]);
    assert_eq!(
        receipt.evaluation.missing_capabilities,
        vec!["resource-refresh"]
    );

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
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"worker-crash-recovery".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerStale));
}
