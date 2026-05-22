use super::super::*;

#[tokio::test]
async fn service_records_non_live_cancel_and_reconcile_order_lifecycle() {
    let store = InMemoryStore::default();
    store
        .upsert_order_lifecycle(&order("order-non-live-cancel", OrderLifecycleState::Posted))
        .await
        .expect("upsert order");
    let service = ExecutorService::new(store.clone());

    let canceled = service
        .record_non_live_cancel_request(
            "acct-1",
            "order-non-live-cancel",
            "operator requested cancel",
            Some("corr-cancel".into()),
        )
        .await
        .expect("record cancel");
    assert_eq!(
        canceled.lifecycle_state,
        OrderLifecycleState::CancelRequested
    );
    let cancel_replay = service
        .record_non_live_cancel_request(
            "acct-1",
            "order-non-live-cancel",
            "operator requested cancel",
            Some("corr-cancel".into()),
        )
        .await
        .expect("replay cancel");
    assert_eq!(
        cancel_replay.lifecycle_state,
        OrderLifecycleState::CancelRequested
    );

    store
        .upsert_order_lifecycle(&order(
            "order-non-live-reconcile",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert remote unknown order");
    let reconciled = service
        .record_non_live_reconcile_observation(
            "order-non-live-reconcile",
            OrderEventKind::ReconcileMissing,
            "remote missing in drill",
            Some("corr-reconcile".into()),
        )
        .await
        .expect("record reconcile")
        .expect("existing order");
    assert_eq!(
        reconciled.lifecycle_state,
        OrderLifecycleState::PartialRemoteUnknown
    );
    store
        .upsert_order_lifecycle(&order(
            "order-non-live-reconcile-unknown",
            OrderLifecycleState::RemoteUnknown,
        ))
        .await
        .expect("upsert remote unknown order");
    let still_unknown = service
        .record_non_live_reconcile_observation(
            "order-non-live-reconcile-unknown",
            OrderEventKind::ReconcileUnknown,
            "remote truth unavailable in drill",
            Some("corr-reconcile-unknown".into()),
        )
        .await
        .expect("record unknown reconcile")
        .expect("existing order");
    assert_eq!(
        still_unknown.lifecycle_state,
        OrderLifecycleState::RemoteUnknown
    );

    let missing = service
        .record_non_live_cancel_request(
            "acct-1",
            "missing-order",
            "operator requested cancel",
            None,
        )
        .await
        .expect_err("missing order must be explicit");
    assert!(matches!(
        missing,
        ServiceError::Store(StoreError::NotFound(_))
    ));

    let cancel_events = store
        .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
            order_id: "order-non-live-cancel".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list cancel events");
    assert_eq!(cancel_events.len(), 1);
    assert_eq!(
        cancel_events[0].payload["kind"],
        "cancel_requested_non_live"
    );
    assert_eq!(cancel_events[0].payload["correlation_id"], "corr-cancel");
    assert_eq!(cancel_events[0].payload["no_remote_side_effect"], true);
    assert!(cancel_events[0].payload.get("raw_signed_payload").is_none());
    assert!(cancel_events[0].payload.get("raw_signature").is_none());

    let reconcile_events = store
        .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
            order_id: "order-non-live-reconcile".into(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("list reconcile events");
    assert_eq!(reconcile_events.len(), 1);
    assert_eq!(
        reconcile_events[0].payload["kind"],
        "reconcile_observed_non_live"
    );
    assert_eq!(
        reconcile_events[0].payload["correlation_id"],
        "corr-reconcile"
    );
    assert_eq!(reconcile_events[0].payload["no_remote_side_effect"], true);
}
