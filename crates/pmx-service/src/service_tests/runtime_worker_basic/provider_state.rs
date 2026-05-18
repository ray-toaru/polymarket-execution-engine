use super::super::*;

#[tokio::test]
async fn store_backed_runtime_provider_uses_store_state() {
    let store = InMemoryStore::default();
    let ready_state = allow_runtime_state();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, ready_state);
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
    assert_eq!(
        snapshot.runtime_state.geoblock_status,
        GeoblockStatus::Allowed
    );
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Healthy);
    assert_eq!(
        snapshot.runtime_state.required_capabilities,
        vec![
            "heartbeat".to_string(),
            "reconcile".to_string(),
            "resource-refresh".to_string(),
        ]
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Allow);
}

#[tokio::test]
async fn service_records_runtime_worker_provider_snapshot_for_decision_gate() {
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

    let receipt = record_runtime_worker_provider_snapshot(
        &store,
        pmx_runtime::RuntimeWorkerProviderSnapshot {
            account_id: "acct-1".into(),
            lease_owner_id: "worker-runtime-1".into(),
            instance_id: "worker-runtime-2".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: Utc::now(),
            provider_name: "real-runtime-provider-test".into(),
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record provider snapshot");
    assert!(receipt.heartbeat_recorded);
    assert!(!receipt.lease_owner_active);
    assert!(!receipt.submit_allowed_by_runtime);
    assert_eq!(receipt.observations_recorded, 6);

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
            .contains(&"heartbeat-lease".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerStale));
}
