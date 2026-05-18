use super::super::super::*;

#[tokio::test]
async fn in_memory_sign_only_client_event_id_replays_and_rejects_mismatch() {
    let store = InMemoryStore::default();
    seed_test_plan(
        &store,
        "exec-sign-only-client-event",
        "acct-sign-only-client-event",
    )
    .await;
    let record = SignOnlyLifecycleRecord {
        execution_id: pmx_core::ExecutionId("exec-sign-only-client-event".into()),
        account_id: pmx_core::AccountId("acct-sign-only-client-event".into()),
        state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
        event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
        client_event_id: Some("client-event-1".into()),
        signed_order_ref: None,
        no_remote_side_effect: true,
        event_id: None,
        created_at: None,
    };
    store
        .record_sign_only_lifecycle_event(&record)
        .await
        .expect("record sign-only lifecycle");
    store
        .record_sign_only_lifecycle_event(&record)
        .await
        .expect("replay client_event_id");
    let mut mismatched = record.clone();
    mismatched.event = pmx_core::SignOnlyLifecycleEventKind::Abandon;
    assert!(matches!(
        store.record_sign_only_lifecycle_event(&mismatched).await,
        Err(StoreError::Conflict(_))
    ));
    let records = store
        .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
            execution_id: "exec-sign-only-client-event".into(),
            limit: 100,
            before_event_id: None,
        })
        .await
        .expect("list sign-only lifecycle");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].client_event_id.as_deref(),
        Some("client-event-1")
    );
}
