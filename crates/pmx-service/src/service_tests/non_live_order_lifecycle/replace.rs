use super::super::*;

#[tokio::test]
async fn service_prepares_replace_idempotently_without_remote_side_effects() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&order("order-replace", OrderLifecycleState::Posted))
        .await
        .unwrap();
    let service = ExecutorService::new(store.clone());

    let prepared = service
        .prepare_non_live_replace(
            "acct-1",
            "order-replace",
            "replacement:sha256:abc123",
            "replace-client-event-1".into(),
        )
        .await
        .unwrap();
    assert_eq!(
        prepared.lifecycle_state,
        OrderLifecycleState::ReplacementPrepared
    );

    let replayed = service
        .prepare_non_live_replace(
            "acct-1",
            "order-replace",
            "replacement:sha256:abc123",
            "replace-client-event-1".into(),
        )
        .await
        .unwrap();
    assert_eq!(replayed, prepared);

    let events = store
        .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
            order_id: "order-replace".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| {
        event.payload["no_remote_side_effect"] == true
            && event.payload.get("raw_signed_payload").is_none()
            && event.payload.get("raw_signature").is_none()
    }));
}

#[tokio::test]
async fn service_rejects_unbound_or_cross_account_replace() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&order("order-replace-reject", OrderLifecycleState::Posted))
        .await
        .unwrap();
    let service = ExecutorService::new(store);

    assert!(
        service
            .prepare_non_live_replace(
                "acct-2",
                "order-replace-reject",
                "replacement:sha256:abc123",
                "replace-client-event-2".into(),
            )
            .await
            .is_err()
    );
    assert!(
        service
            .prepare_non_live_replace(
                "acct-1",
                "order-replace-reject",
                "raw-order-payload",
                "replace-client-event-3".into(),
            )
            .await
            .is_err()
    );
}
