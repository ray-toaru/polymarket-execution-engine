use super::super::*;
use chrono::Utc;
use pmx_core::{OrderEventKind, OrderLifecycleState};

#[tokio::test]
async fn postgres_order_lifecycle_replays_same_correlation_id() {
    let Some(store) = test_store().await else {
        return;
    };
    let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let account = format!("acct-order-life-replay-{suffix}");
    let execution = format!("exec-order-life-replay-{suffix}");
    seed_execution_plan(&store, &account, &execution).await;
    let order_id = format!("order-life-replay-{suffix}");
    store
        .upsert_order_lifecycle(&OrderLifecycleRecord {
            order_id: order_id.clone(),
            execution_id: execution,
            account_id: account,
            condition_id: "cond-order-life-replay".into(),
            token_id: "token-order-life-replay".into(),
            side: "BUY".into(),
            lifecycle_state: OrderLifecycleState::Posted,
            remote_order_id: Some(format!("remote-{order_id}")),
            remote_state: Some("OPEN".into()),
            created_at: None,
            updated_at: None,
        })
        .await
        .expect("upsert order");
    let event = OrderLifecycleEventRecord {
        event_id: None,
        order_id: order_id.clone(),
        event: OrderEventKind::CancelRequested,
        event_source: "pmx-store-test".into(),
        correlation_id: Some(format!("corr-pg-order-life-replay-{suffix}")),
        payload: serde_json::json!({"no_remote_side_effect": true}),
        created_at: None,
    };
    store
        .record_order_lifecycle_event(&event)
        .await
        .expect("record order event");
    let replayed = store
        .record_order_lifecycle_event(&event)
        .await
        .expect("replay order event");
    assert_eq!(
        replayed.lifecycle_state,
        OrderLifecycleState::CancelRequested
    );
    let mut mismatched = event.clone();
    mismatched.event = OrderEventKind::ReconcileOpen;
    assert!(matches!(
        store.record_order_lifecycle_event(&mismatched).await,
        Err(StoreError::Conflict(_))
    ));
    let mut payload_mismatch = event.clone();
    payload_mismatch.event = OrderEventKind::CancelRequested;
    payload_mismatch.payload = serde_json::json!({"no_remote_side_effect": false});
    assert!(matches!(
        store.record_order_lifecycle_event(&payload_mismatch).await,
        Err(StoreError::Conflict(_))
    ));
    let events = store
        .list_order_lifecycle_events(&OrderLifecycleEventQuery {
            order_id,
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list order events");
    assert_eq!(events.len(), 1);
}
