use super::*;

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
