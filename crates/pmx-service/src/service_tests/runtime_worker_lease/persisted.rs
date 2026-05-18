use super::super::*;

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
