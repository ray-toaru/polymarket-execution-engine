use super::super::*;

#[tokio::test]
async fn service_records_reconcile_backlog_worker_tick_for_decision_gate() {
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
    let receipt = record_reconcile_backlog_worker_tick(
        &store,
        ReconcileBacklogWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "reconcile-backlog-worker-test".into(),
            instance_id: "worker-reconcile-backlog".into(),
            lease_owner_id: "worker-reconcile-backlog".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_order_ids: vec!["order-remote-unknown".into()],
            observed_at,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record reconcile backlog worker tick");
    assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
    assert!(receipt.evaluation.submit_blocked);
    assert!(receipt.provider_tick.lease_owner_active);
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
            .contains(&"reconcile-backlog".to_string())
    );
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
    assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
}

#[tokio::test]
async fn service_records_reconcile_backlog_from_order_lifecycle() {
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
    store
        .upsert_order_lifecycle(&order(
            "order-lifecycle-backlog",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert remote unknown order");
    store
        .upsert_order_lifecycle(&order(
            "order-lifecycle-posted",
            OrderLifecycleState::Posted,
        ))
        .await
        .expect("upsert posted order");

    let observed_at = Utc::now();
    let receipt = record_reconcile_backlog_from_order_lifecycle(
        &store,
        ReconcileBacklogWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "reconcile-lifecycle-reader-test".into(),
            instance_id: "worker-reconcile-lifecycle-reader".into(),
            lease_owner_id: "worker-reconcile-lifecycle-reader".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_order_ids: vec![],
            observed_at,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record reconcile backlog from order lifecycle");
    assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
    assert!(receipt.evaluation.submit_blocked);
    assert!(!receipt.provider_tick.submit_allowed_by_runtime);

    let service = ExecutorService::with_runtime_provider(
        store.clone(),
        StoreBackedRuntimeStateProvider::new(store.clone()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"reconcile-backlog".to_string())
    );
}
