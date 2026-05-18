use super::*;

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
