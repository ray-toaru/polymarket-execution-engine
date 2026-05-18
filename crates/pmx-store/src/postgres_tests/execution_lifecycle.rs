use super::*;

#[tokio::test]
async fn postgres_records_execution_lifecycle_event() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-life");
    let execution = unique("exec-life");
    seed_execution_plan(&store, &account, &execution).await;
    store
        .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
            event_id: None,
            execution_id: execution.clone(),
            account_id: account.clone(),
            event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
            event_source: "pmx-service".into(),
            payload: serde_json::json!({"no_remote_side_effect": true}),
            created_at: None,
        })
        .await
        .expect("record lifecycle event");
    let client = store.client().await.expect("test postgres client");
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM execution_lifecycle_events WHERE execution_id = $1 AND event_type = 'SUBMIT_BLOCKED_BEFORE_REMOTE'",
            &[&execution],
        )
        .await
        .expect("count lifecycle events")
        .get(0);
    assert_eq!(count, 1);
}

#[tokio::test]
async fn postgres_records_cancel_reconcile_lifecycle_events() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct-cancel-life");
    let execution = unique("exec-cancel-life");
    seed_execution_plan(&store, &account, &execution).await;
    for event_type in ["CANCEL_REQUESTED_NON_LIVE", "RECONCILE_REQUESTED_NON_LIVE"] {
        store
            .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                event_id: None,
                execution_id: execution.clone(),
                account_id: account.clone(),
                event_type: event_type.into(),
                event_source: "pmx-store-test".into(),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record lifecycle event");
    }
    let events = store
        .list_execution_lifecycle_events(&ExecutionLifecycleQuery {
            execution_id: execution.clone(),
            limit: 100,
            before_event_id: None,
        })
        .await
        .expect("list lifecycle events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "CANCEL_REQUESTED_NON_LIVE");
    assert_eq!(events[1].event_type, "RECONCILE_REQUESTED_NON_LIVE");
}
