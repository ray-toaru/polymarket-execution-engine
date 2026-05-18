use super::super::super::*;

#[tokio::test]
async fn in_memory_rejects_sign_only_for_unknown_execution() {
    let store = InMemoryStore::default();
    let record = SignOnlyLifecycleRecord {
        execution_id: pmx_core::ExecutionId("missing-exec".into()),
        account_id: pmx_core::AccountId("acct-missing-exec".into()),
        state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
        event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
        client_event_id: Some("missing-exec-event".into()),
        signed_order_ref: None,
        no_remote_side_effect: true,
        event_id: None,
        created_at: None,
    };
    assert!(matches!(
        store.record_sign_only_lifecycle_event(&record).await,
        Err(StoreError::NotFound(_))
    ));
}

#[tokio::test]
async fn in_memory_rejects_sign_only_remote_side_effect_records() {
    let store = InMemoryStore::default();
    seed_test_plan(&store, "exec-sign-only", "acct-sign-only").await;
    let record = SignOnlyLifecycleRecord {
        execution_id: pmx_core::ExecutionId("exec-sign-only".into()),
        account_id: pmx_core::AccountId("acct-sign-only".into()),
        state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
        event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
        client_event_id: None,
        signed_order_ref: Some("sign-only:redacted-ref".into()),
        no_remote_side_effect: false,
        event_id: None,
        created_at: None,
    };
    assert!(
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .is_err()
    );
}
