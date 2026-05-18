use super::super::*;

#[tokio::test]
async fn postgres_records_runtime_worker_observation() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-worker-observation");
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: account.clone(),
            capability: "heartbeat-lease".into(),
            worker_kind: "HeartbeatLease".into(),
            status: "STALE".into(),
            should_fail_closed: true,
            reason: "lease expired".into(),
            observed_at: None,
        })
        .await
        .expect("record runtime worker observation");
    let client = store.client().await.expect("test postgres client");
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM runtime_worker_observations WHERE account_id = $1",
            &[&account],
        )
        .await
        .expect("count runtime worker observations")
        .get(0);
    assert_eq!(count, 1);
}
