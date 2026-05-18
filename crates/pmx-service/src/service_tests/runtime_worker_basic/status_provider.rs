use super::super::*;

#[tokio::test]
async fn service_lists_runtime_worker_status() {
    let store = InMemoryStore::default();
    let observed_at = Utc::now();
    record_runtime_worker_tick(
        &store,
        "acct-1",
        RuntimeWorkerTick {
            worker_id: "worker-status-query".into(),
            role: "HeartbeatLease".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_error: None,
            signals: vec![RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: Some(observed_at),
                last_error: Some("lease expired".into()),
            }],
        },
    )
    .await
    .expect("record worker tick");
    let service = ExecutorService::new(store);
    let report = service
        .list_runtime_worker_status(RuntimeWorkerStatusQuery {
            account_id: "acct-1".into(),
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert_eq!(report.heartbeats.len(), 1);
    assert_eq!(report.heartbeats[0].worker_id, "worker-status-query");
    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.observations[0].capability, "heartbeat-lease");
    assert!(report.observations[0].should_fail_closed);
}
