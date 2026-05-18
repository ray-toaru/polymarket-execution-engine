use super::super::*;
use pmx_core::sign_only_lifecycle_records_equivalent;

#[tokio::test]
async fn in_memory_persists_sign_only_lifecycle_records() {
    let store = InMemoryStore::default();
    let execution_id = pmx_core::ExecutionId("exec-sign-only".into());
    let account_id = pmx_core::AccountId("acct-sign-only".into());
    seed_test_plan(&store, &execution_id.0, &account_id.0).await;
    let records_to_append = [
        SignOnlyLifecycleRecord {
            execution_id: execution_id.clone(),
            account_id: account_id.clone(),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: execution_id.clone(),
            account_id: account_id.clone(),
            state: pmx_core::SignOnlyLifecycleState::SigningRequested,
            event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: execution_id.clone(),
            account_id: account_id.clone(),
            state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
            event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some("sign-only:redacted-ref".into()),
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
    ];
    for record in &records_to_append {
        store
            .record_sign_only_lifecycle_event(record)
            .await
            .expect("record sign-only lifecycle");
    }
    let records = store
        .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
            execution_id: "exec-sign-only".into(),
            limit: 100,
            before_event_id: None,
        })
        .await
        .expect("list sign-only lifecycle");
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|record| record.event_id.is_some()));
    assert!(records.iter().all(|record| record.created_at.is_some()));
    assert!(sign_only_lifecycle_records_equivalent(
        records.last().unwrap(),
        records_to_append.last().unwrap()
    ));
}

#[tokio::test]
async fn in_memory_sign_only_replay_is_idempotent() {
    let store = InMemoryStore::default();
    seed_test_plan(&store, "exec-sign-only-replay", "acct-sign-only-replay").await;
    let record = SignOnlyLifecycleRecord {
        execution_id: pmx_core::ExecutionId("exec-sign-only-replay".into()),
        account_id: pmx_core::AccountId("acct-sign-only-replay".into()),
        state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
        event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
        client_event_id: None,
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
        .expect("replay sign-only lifecycle");
    let records = store
        .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
            execution_id: "exec-sign-only-replay".into(),
            limit: 100,
            before_event_id: None,
        })
        .await
        .expect("list sign-only lifecycle");
    assert_eq!(records.len(), 1);
}

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
