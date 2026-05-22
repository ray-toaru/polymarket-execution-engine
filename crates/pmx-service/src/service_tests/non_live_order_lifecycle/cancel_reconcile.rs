use super::super::*;
use pmx_gateway::{ClobGateway, SignerProvider};

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

#[tokio::test]
async fn service_live_cancel_gateway_records_remote_acceptance_and_unknown() {
    let store = InMemoryStore::default();
    let posted = order("order-live-cancel", OrderLifecycleState::Posted);
    store
        .upsert_order_lifecycle(&posted)
        .await
        .expect("upsert order");
    let gateway = pmx_gateway::FakeGateway::new();
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let signed = signer
        .signer_for_account(&AccountId("acct-1".into()))
        .await
        .expect("signer")
        .sign_order(&pmx_gateway::PlanOrder {
            execution_id: "exec-order-life".into(),
            account_id: AccountId("acct-1".into()),
            token_id: TokenId("token-1".into()),
            side: "Buy".into(),
            limit_price: "0.5".into(),
            size: "5".into(),
            time_in_force: "Fok".into(),
        })
        .await
        .expect("sign");
    let ack = gateway.post_order(&signed).await.expect("post fake order");
    let mut posted_with_remote = posted.clone();
    posted_with_remote.remote_order_id = Some(ack.remote_order_id.0);
    store
        .upsert_order_lifecycle(&posted_with_remote)
        .await
        .expect("upsert remote order id");
    let service = ExecutorService::new(store.clone());

    let accepted = service
        .cancel_order_with_gateway(
            LiveCancelCommand {
                account_id: "acct-1".into(),
                order_id: "order-live-cancel".into(),
                reason: "operator cancel fallback".into(),
                correlation_id: Some("corr-live-cancel".into()),
            },
            &gateway,
        )
        .await
        .expect("live cancel accepted");
    assert_eq!(
        accepted.lifecycle_state,
        OrderLifecycleState::CancelRemoteAccepted
    );

    store
        .upsert_order_lifecycle(&order(
            "order-live-cancel-unknown",
            OrderLifecycleState::Posted,
        ))
        .await
        .expect("upsert unknown order");
    let mut unknown = store
        .load_order_lifecycle("order-live-cancel-unknown")
        .await
        .expect("load")
        .expect("order");
    unknown.remote_order_id = Some("remote-missing-order".into());
    store
        .upsert_order_lifecycle(&unknown)
        .await
        .expect("upsert missing remote id");
    let unknown = service
        .cancel_order_with_gateway(
            LiveCancelCommand {
                account_id: "acct-1".into(),
                order_id: "order-live-cancel-unknown".into(),
                reason: "operator cancel fallback".into(),
                correlation_id: Some("corr-live-cancel-unknown".into()),
            },
            &gateway,
        )
        .await
        .expect("live cancel remote unknown");
    assert_eq!(unknown.lifecycle_state, OrderLifecycleState::RemoteUnknown);
}
