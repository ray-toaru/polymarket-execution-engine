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
            order_id: "order-life-replay".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list events");
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn in_memory_replace_lifecycle_is_idempotent_by_correlation_id() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&test_order("order-replace-replay"))
        .await
        .expect("upsert order");
    let event = OrderLifecycleEventRecord {
        event_id: None,
        order_id: "order-replace-replay".into(),
        event: OrderEventKind::ReplaceRequested,
        event_source: "pmx-store-test".into(),
        correlation_id: Some("replace-client-event-1".into()),
        payload: serde_json::json!({
            "replacement_ref": "replacement:sha256:test",
            "no_remote_side_effect": true
        }),
        created_at: None,
    };
    let first = store
        .record_order_lifecycle_event(&event)
        .await
        .expect("record replace request");
    let replay = store
        .record_order_lifecycle_event(&event)
        .await
        .expect("replay replace request");
    assert_eq!(first, replay);
    assert_eq!(
        replay.lifecycle_state,
        OrderLifecycleState::ReplaceRequested
    );
    assert_eq!(
        store
            .list_order_lifecycle_events(&OrderLifecycleEventQuery {
                order_id: "order-replace-replay".into(),
                limit: 10,
                before_event_id: None,
            })
            .await
            .unwrap()
            .len(),
        1
    );
}
