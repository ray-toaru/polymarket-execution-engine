use super::super::*;

#[tokio::test]
async fn service_records_runtime_worker_signals_for_decision_gate() {
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
    let recorded = record_runtime_worker_signals(
        &store,
        "acct-1",
        &[RuntimeSignal::HeartbeatLease {
            active: false,
            last_observed_at: Some(Utc::now()),
            last_error: Some("lease expired".into()),
        }],
    )
    .await
    .expect("record runtime worker signal");
    assert_eq!(recorded, 1);

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
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerStale));
}

#[tokio::test]
async fn service_records_runtime_worker_tick_heartbeat_and_observations() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    let receipt = record_runtime_worker_tick(
        &store,
        "acct-1",
        RuntimeWorkerTick {
            worker_id: "worker-websocket-market".into(),
            role: "WebSocketLiveness".into(),
            capability: "websocket:market".into(),
            status: "HEALTHY".into(),
            last_error: None,
            signals: vec![RuntimeSignal::WebSocket {
                channel: pmx_runtime::WebSocketChannel::Market,
                connected: false,
                stale: true,
                last_observed_at: Some(Utc::now()),
                last_error: Some("market websocket disconnected".into()),
            }],
        },
    )
    .await
    .expect("record runtime worker tick");
    assert!(receipt.heartbeat_recorded);
    assert_eq!(receipt.observations_recorded, 1);

    let state = store
        .load_runtime_state(&RuntimeStateQuery {
            account_id: "acct-1".into(),
            condition_id: "cond-1".into(),
            collateral_profile_id: None,
            required_capabilities: vec!["websocket:market".into()],
        })
        .await
        .expect("runtime state");
    assert_eq!(state.worker_status, WorkerStatus::Degraded);

    let normalized = ExecutorService::new(store.clone())
        .normalize(intent())
        .await
        .expect("normalize");
    let decision = evaluate_constraints(
        &normalized,
        &FeasibilitySnapshot {
            snapshot_id: "snapshot-worker-tick".into(),
            snapshot_hash: HashValue("snapshot-hash-worker-tick".into()),
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            correlation_id: None,
            runtime_state: state,
            captured_at: Utc::now(),
        },
    );
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
}
