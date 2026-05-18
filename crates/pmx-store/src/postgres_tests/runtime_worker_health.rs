use super::*;
use chrono::Utc;

#[tokio::test]
async fn postgres_records_worker_heartbeat() {
    let Some(store) = test_store().await else {
        return;
    };
    let worker_id = format!(
        "worker-heartbeat-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: worker_id.clone(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now(),
            last_error: None,
        })
        .await
        .expect("record heartbeat");
    let client = store.client().await.expect("test postgres client");
    let row = client
        .query_one(
            "SELECT status FROM worker_health WHERE worker_id = $1",
            &[&worker_id],
        )
        .await
        .expect("heartbeat row");
    let status: String = row.get(0);
    assert_eq!(status, "HEALTHY");
}

#[tokio::test]
async fn postgres_lists_runtime_worker_status() {
    let Some(store) = test_store().await else {
        return;
    };
    let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let worker_id = format!("worker-status-query-{suffix}");
    let account_id = format!("acct-status-query-{suffix}");
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: worker_id.clone(),
            role: "Heartbeat".into(),
            capability: "heartbeat".into(),
            status: "HEALTHY".into(),
            last_heartbeat_at: Utc::now(),
            last_error: None,
        })
        .await
        .expect("record heartbeat");
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: account_id.clone(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: None,
        })
        .await
        .expect("record observation");
    let report = store
        .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
            account_id,
            limit: 100,
            before_observed_at: None,
        })
        .await
        .expect("list runtime worker status");
    assert!(
        report
            .heartbeats
            .iter()
            .any(|heartbeat| heartbeat.worker_id == worker_id)
    );
    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.observations[0].status, "STALE");
    assert!(report.observations[0].should_fail_closed);
}
