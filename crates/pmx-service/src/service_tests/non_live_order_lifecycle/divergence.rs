use super::super::*;

#[tokio::test]
async fn service_classifies_and_records_order_lifecycle_divergence_without_remote_side_effect() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&order(
            "order-divergence",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert order");
    let service = ExecutorService::new(store.clone());

    let (first_divergence, first_update) = service
        .reconcile_order_lifecycle_divergence(
            "order-divergence",
            Some("acct-1"),
            RemoteOrderObservation::Missing,
            "remote read observed missing",
            Some("corr-divergence-1".into()),
        )
        .await
        .expect("first divergence")
        .expect("order exists");
    assert_eq!(
        first_divergence.kind,
        OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing
    );
    assert!(!first_divergence.operator_required);
    assert!(first_divergence.no_remote_side_effect);
    assert_eq!(
        first_update.expect("first update").lifecycle_state,
        OrderLifecycleState::PartialRemoteUnknown
    );

    let (second_divergence, second_update) = service
        .reconcile_order_lifecycle_divergence(
            "order-divergence",
            Some("acct-1"),
            RemoteOrderObservation::Missing,
            "remote read still missing",
            Some("corr-divergence-2".into()),
        )
        .await
        .expect("second divergence")
        .expect("order exists");
    assert!(second_divergence.operator_required);
    assert_eq!(
        second_update.expect("second update").lifecycle_state,
        OrderLifecycleState::Failed
    );
    store
        .upsert_order_lifecycle(&order(
            "order-divergence-unknown",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert unknown order");
    let (unknown_divergence, unknown_update) = service
        .reconcile_order_lifecycle_divergence(
            "order-divergence-unknown",
            Some("acct-1"),
            RemoteOrderObservation::Unknown,
            "remote read stayed unknown",
            Some("corr-divergence-unknown".into()),
        )
        .await
        .expect("unknown divergence")
        .expect("order exists");
    assert_eq!(
        unknown_divergence.kind,
        OrderLifecycleDivergenceKind::LocalRemoteUnknownStillUnknown
    );
    assert!(unknown_divergence.operator_required);
    assert_eq!(
        unknown_update.expect("unknown update").lifecycle_state,
        OrderLifecycleState::RemoteUnknown
    );

    let events = store
        .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
            order_id: "order-divergence".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("order lifecycle events");
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .all(|event| event.payload["no_remote_side_effect"] == true)
    );
    assert!(
        events
            .iter()
            .all(|event| event.payload["kind"] == "order_lifecycle_divergence_non_live")
    );
    assert!(
        events
            .iter()
            .all(|event| event.payload.get("raw_signed_payload").is_none())
    );
    assert!(
        events
            .iter()
            .all(|event| event.correlation_id.as_deref().is_some())
    );

    let queried = service
        .list_order_lifecycle_events(pmx_store::OrderLifecycleEventQuery {
            order_id: "order-divergence".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("query order lifecycle events");
    assert_eq!(queried, events);
}
