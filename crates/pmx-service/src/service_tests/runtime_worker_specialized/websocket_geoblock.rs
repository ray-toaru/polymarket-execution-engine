use super::super::*;

#[tokio::test]
async fn service_records_websocket_liveness_worker_tick_for_decision_gate() {
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
    let receipt = record_websocket_liveness_worker_tick(
        &store,
        WebSocketLivenessWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "websocket-liveness-worker-test".into(),
            instance_id: "worker-websocket-liveness".into(),
            lease_owner_id: "worker-websocket-liveness".into(),
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::WebSocketLivenessObservation {
                    channel: pmx_runtime::WebSocketChannel::Market,
                    connected: true,
                    last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
                pmx_runtime::WebSocketLivenessObservation {
                    channel: pmx_runtime::WebSocketChannel::User,
                    connected: false,
                    last_message_at: None,
                    status: pmx_runtime::HealthLevel::Degraded,
                    last_error: Some("user websocket disconnected".into()),
                },
            ],
        },
    )
    .await
    .expect("record websocket liveness worker tick");
    assert!(receipt.evaluation.market_connected);
    assert!(!receipt.evaluation.user_connected);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

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
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"websocket:user".to_string())
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
    assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
}

#[tokio::test]
async fn service_records_geoblock_worker_tick_for_decision_gate() {
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

    let receipt = record_geoblock_worker_tick(
        &store,
        GeoblockWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "geoblock-worker-test".into(),
            instance_id: "worker-geoblock".into(),
            lease_owner_id: "worker-geoblock".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            status: GeoblockStatus::Unknown,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: Utc::now(),
            last_error: Some("geoblock provider timeout".into()),
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record geoblock worker tick");
    assert!(!receipt.evaluation.submit_allowed);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

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
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Unknown);
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerUnknown));
}
