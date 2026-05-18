use super::*;
use pmx_core::{OrderEventKind, OrderLifecycleState};

fn test_order(order_id: &str) -> OrderLifecycleRecord {
    OrderLifecycleRecord {
        order_id: order_id.into(),
        execution_id: format!("exec-{order_id}"),
        account_id: "acct-order-life".into(),
        condition_id: "cond-order-life".into(),
        token_id: "token-order-life".into(),
        side: "BUY".into(),
        lifecycle_state: OrderLifecycleState::Posted,
        remote_order_id: Some(format!("remote-{order_id}")),
        remote_state: Some("OPEN".into()),
        created_at: None,
        updated_at: None,
    }
}

#[tokio::test]
async fn in_memory_order_lifecycle_records_cancel_requested() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&test_order("order-life-1"))
        .await
        .expect("upsert order");
    let updated = store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: "order-life-1".into(),
            event: OrderEventKind::CancelRequested,
            event_source: "pmx-store-test".into(),
            correlation_id: Some("corr-order-life-1".into()),
            payload: serde_json::json!({"no_remote_side_effect": true}),
            created_at: None,
        })
        .await
        .expect("record event");
    assert_eq!(
        updated.lifecycle_state,
        OrderLifecycleState::CancelRequested
    );
    let events = store
        .list_order_lifecycle_events(&OrderLifecycleEventQuery {
            order_id: "order-life-1".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, OrderEventKind::CancelRequested);
    assert_eq!(
        events[0].correlation_id.as_deref(),
        Some("corr-order-life-1")
    );
    assert!(events[0].event_id.is_some());
}

#[tokio::test]
async fn in_memory_order_lifecycle_replays_same_correlation_id() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&test_order("order-life-replay"))
        .await
        .expect("upsert order");
    let event = OrderLifecycleEventRecord {
        event_id: None,
        order_id: "order-life-replay".into(),
        event: OrderEventKind::CancelRequested,
        event_source: "pmx-store-test".into(),
        correlation_id: Some("corr-order-life-replay".into()),
        payload: serde_json::json!({"no_remote_side_effect": true}),
        created_at: None,
    };
    store
        .record_order_lifecycle_event(&event)
        .await
        .expect("record event");
    let replayed = store
        .record_order_lifecycle_event(&event)
        .await
        .expect("replay event");
    assert_eq!(
        replayed.lifecycle_state,
        OrderLifecycleState::CancelRequested
    );
    let mut mismatched = event;
    mismatched.event = OrderEventKind::ReconcileOpen;
    assert!(matches!(
        store.record_order_lifecycle_event(&mismatched).await,
        Err(StoreError::Conflict(_))
    ));
    let events = store
        .list_order_lifecycle_events(&OrderLifecycleEventQuery {
            order_id: "order-life-replay".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list events");
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn in_memory_order_lifecycle_rejects_invalid_transition() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&test_order("order-life-invalid"))
        .await
        .expect("upsert order");
    let err = store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: "order-life-invalid".into(),
            event: OrderEventKind::CancelConfirmed,
            event_source: "pmx-store-test".into(),
            correlation_id: None,
            payload: serde_json::json!({}),
            created_at: None,
        })
        .await
        .expect_err("invalid transition");
    assert!(matches!(err, StoreError::Conflict(_)));
}

#[tokio::test]
async fn in_memory_lists_reconcile_backlog_orders() {
    let store = InMemoryStore::default();
    let mut remote_unknown = test_order("order-reconcile-backlog-1");
    remote_unknown.lifecycle_state = OrderLifecycleState::RemoteUnknown;
    let mut partial_remote_unknown = test_order("order-reconcile-backlog-2");
    partial_remote_unknown.lifecycle_state = OrderLifecycleState::PartialRemoteUnknown;
    let posted = test_order("order-reconcile-backlog-posted");
    for order in [&remote_unknown, &partial_remote_unknown, &posted] {
        store
            .upsert_order_lifecycle(order)
            .await
            .expect("upsert order");
    }
    let backlog = store
        .list_reconcile_backlog_orders(&OrderReconcileBacklogQuery {
            account_id: "acct-order-life".into(),
            limit: 100,
        })
        .await
        .expect("list reconcile backlog");
    let order_ids: Vec<_> = backlog
        .iter()
        .map(|order| order.order_id.as_str())
        .collect();
    assert_eq!(order_ids.len(), 2);
    assert!(order_ids.contains(&"order-reconcile-backlog-1"));
    assert!(order_ids.contains(&"order-reconcile-backlog-2"));
}
