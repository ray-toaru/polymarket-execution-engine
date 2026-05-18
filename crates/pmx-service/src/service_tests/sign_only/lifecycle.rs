use super::super::*;

#[tokio::test]
async fn service_validates_and_persists_sign_only_lifecycle_sequence() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    let execution_id = ExecutionId("exec-sign-only-service".into());
    let account_id = AccountId("acct-sign-only-service".into());
    seed_test_plan(&store, &execution_id.0, &account_id.0).await;
    for (event, state, signed_order_ref) in [
        (
            SignOnlyLifecycleEventKind::PrepareReservation,
            SignOnlyLifecycleState::ReservationPrepared,
            None,
        ),
        (
            SignOnlyLifecycleEventKind::RequestSigning,
            SignOnlyLifecycleState::SigningRequested,
            None,
        ),
        (
            SignOnlyLifecycleEventKind::SignedWithoutPost,
            SignOnlyLifecycleState::SignedDryRun,
            Some("sign-only:redacted-ref".to_string()),
        ),
    ] {
        service
            .record_sign_only_lifecycle_event(SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state,
                event,
                client_event_id: None,
                signed_order_ref,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            })
            .await
            .expect("record sign-only lifecycle");
    }
    let records = service
        .list_sign_only_lifecycle_events(SignOnlyLifecycleQuery {
            execution_id: execution_id.0.clone(),
            limit: 100,
            before_event_id: None,
        })
        .await
        .expect("list sign-only lifecycle");
    assert_eq!(records.len(), 3);
    assert_eq!(
        records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
}

#[tokio::test]
async fn service_rejects_sign_only_sequence_mismatch() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    seed_test_plan(&store, "exec-sign-only-bad", "acct-sign-only-bad").await;
    let err = service
        .record_sign_only_lifecycle_event(SignOnlyLifecycleRecord {
            execution_id: ExecutionId("exec-sign-only-bad".into()),
            account_id: AccountId("acct-sign-only-bad".into()),
            state: SignOnlyLifecycleState::SignedDryRun,
            event: SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some("sign-only:redacted-ref".into()),
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        })
        .await
        .expect_err("cannot sign without reservation/signing request");
    assert!(matches!(err, ServiceError::Conflict(_)));
}
