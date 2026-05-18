use super::*;

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
