use super::*;

#[tokio::test]
async fn service_records_continuous_runtime_worker_ticks_fail_closed_on_any_bad_snapshot() {
    let store = InMemoryStore::default();
    let observed_at = Utc::now();
    let healthy_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
        account_id: "acct-1".into(),
        lease_owner_id: "worker-runtime-1".into(),
        instance_id: "worker-runtime-1".into(),
        market_websocket_connected: true,
        market_websocket_stale: false,
        user_websocket_connected: true,
        user_websocket_stale: false,
        geoblock_status: GeoblockStatus::Allowed,
        resource_refresh_fresh: true,
        remote_unknown_orders: 0,
        observed_at,
        provider_name: "runtime-provider-test".into(),
        no_trading_side_effect: true,
    };
    let stale_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
        instance_id: "worker-runtime-2".into(),
        market_websocket_stale: true,
        observed_at: observed_at + chrono::Duration::seconds(1),
        ..healthy_snapshot.clone()
    };

    let receipt = record_runtime_worker_continuous_tick(
        &store,
        RuntimeWorkerContinuousTick {
            snapshots: vec![healthy_snapshot, stale_snapshot],
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record continuous runtime ticks");

    assert_eq!(receipt.ticks_recorded.len(), 2);
    assert!(receipt.ticks_recorded[0].submit_allowed_by_runtime);
    assert!(!receipt.ticks_recorded[1].submit_allowed_by_runtime);
    assert!(!receipt.all_submit_allowed_by_runtime);
    assert!(receipt.no_trading_side_effect);

    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id: "acct-1".into(),
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert_eq!(report.heartbeats.len(), 2);
    assert!(
        report
            .observations
            .iter()
            .any(|observation| observation.capability == "websocket:market"
                && observation.should_fail_closed)
    );
}

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
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Block);
}

#[tokio::test]
async fn service_records_heartbeat_lease_from_persisted_worker_status() {
    let store = InMemoryStore::default();
    store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
    let observed_at = Utc::now();
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: "worker-a".into(),
            role: "HeartbeatLease".into(),
            capability: "heartbeat-lease".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: observed_at - chrono::Duration::seconds(2),
            last_error: None,
        })
        .await
        .expect("record existing lease heartbeat");

    let receipt = record_heartbeat_lease_from_worker_status(
        &store,
        HeartbeatLeaseStoreTick {
            account_id: "acct-1".into(),
            provider_name: "heartbeat-lease-store-test".into(),
            instance_id: "worker-b".into(),
            observed_at,
            stale_after_seconds: 30,
            status: "HEALTHY".into(),
            last_error: None,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record heartbeat lease from persisted status");
    assert_eq!(receipt.election.lease_owner_id, "worker-b");
    assert!(receipt.election.lease_owner_active);
    assert!(receipt.provider_tick.submit_allowed_by_runtime);
    assert_eq!(receipt.candidates_loaded, 2);
    assert!(receipt.heartbeat_recorded);

    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id: "acct-1".into(),
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert!(
        report
            .heartbeats
            .iter()
            .any(|heartbeat| heartbeat.worker_id == "worker-b"
                && heartbeat.capability == "heartbeat-lease")
    );
    assert!(
        report
            .observations
            .iter()
            .any(|observation| observation.capability == "heartbeat-lease"
                && !observation.should_fail_closed)
    );
}

#[tokio::test]
async fn service_records_heartbeat_lease_from_postgres_worker_status() {
    let Ok(database_url) = std::env::var("PMX_TEST_DATABASE_URL") else {
        eprintln!(
            "PMX_TEST_DATABASE_URL not set; skipping service PostgreSQL heartbeat lease test"
        );
        return;
    };
    let store = PostgresStore::connect(database_url)
        .await
        .expect("connect postgres");
    store.apply_schema().await.expect("apply postgres schema");
    let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let account_id = format!("acct-heartbeat-lease-pg-{suffix}");
    let existing_worker = format!("worker-heartbeat-lease-pg-a-{suffix}");
    let local_worker = format!("worker-heartbeat-lease-pg-b-{suffix}");
    let observed_at = Utc::now();
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: existing_worker,
            role: "HeartbeatLease".into(),
            capability: "heartbeat-lease".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: observed_at - chrono::Duration::seconds(2),
            last_error: None,
        })
        .await
        .expect("record existing postgres heartbeat");

    let receipt = record_heartbeat_lease_from_worker_status(
        &store,
        HeartbeatLeaseStoreTick {
            account_id: account_id.clone(),
            provider_name: "heartbeat-lease-postgres-test".into(),
            instance_id: local_worker.clone(),
            observed_at,
            stale_after_seconds: 30,
            status: "HEALTHY".into(),
            last_error: None,
            no_trading_side_effect: true,
        },
    )
    .await
    .expect("record postgres heartbeat lease from persisted status");

    assert_eq!(receipt.election.lease_owner_id, local_worker);
    assert!(receipt.election.lease_owner_active);
    assert!(receipt.provider_tick.submit_allowed_by_runtime);
    assert!(receipt.candidates_loaded >= 2);
    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id,
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list postgres runtime worker status");
    assert!(
        report
            .observations
            .iter()
            .any(|observation| observation.capability == "heartbeat-lease"
                && !observation.should_fail_closed)
    );
}
