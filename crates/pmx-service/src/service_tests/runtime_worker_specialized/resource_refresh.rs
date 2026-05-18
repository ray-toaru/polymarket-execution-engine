use super::super::*;

#[tokio::test]
async fn service_records_resource_refresh_worker_tick_for_decision_gate() {
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
    let receipt = record_resource_refresh_worker_tick(
        &store,
        ResourceRefreshWorkerTick {
            account_id: "acct-1".into(),
            provider_name: "resource-refresh-worker-test".into(),
            instance_id: "worker-resource-refresh".into(),
            lease_owner_id: "worker-resource-refresh".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            remote_unknown_orders: 0,
            observed_at,
            stale_after_seconds: 30,
            no_trading_side_effect: true,
            observations: vec![
                pmx_runtime::ResourceRefreshObservation {
                    component: pmx_runtime::ResourceRefreshComponent::Account,
                    resource_id: "acct-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(60),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
                pmx_runtime::ResourceRefreshObservation {
                    component: pmx_runtime::ResourceRefreshComponent::Market,
                    resource_id: "cond-1".into(),
                    refreshed_at: observed_at - chrono::Duration::seconds(5),
                    status: pmx_runtime::HealthLevel::Healthy,
                    last_error: None,
                },
            ],
        },
    )
    .await
    .expect("record resource refresh worker tick");
    assert!(!receipt.evaluation.fresh);
    assert_eq!(receipt.evaluation.stale_components, vec!["account:acct-1"]);
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
    assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
    assert!(
        snapshot
            .runtime_state
            .required_capabilities
            .contains(&"resource-refresh".to_string())
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
