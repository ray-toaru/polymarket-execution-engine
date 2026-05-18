use super::*;
use pmx_core::sign_only_lifecycle_records_equivalent;

#[tokio::test]
async fn in_memory_admin_audit_records_without_exposing_secrets() {
    let store = InMemoryStore::default();
    store
        .record_admin_audit_event(&AdminAuditEvent {
            audit_id: None,
            principal_subject: "admin-token".into(),
            operation: "KillSwitch".into(),
            request_fingerprint: Some("abc123".into()),
            correlation_id: Some("corr-admin-test".into()),
            result: "ACCEPTED".into(),
            created_at: None,
        })
        .await
        .expect("record audit event");
    let len = store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .admin_audit
        .len();
    assert_eq!(len, 1);
}

#[tokio::test]
async fn in_memory_admin_audit_paginates_and_filters_by_cursor() {
    let store = InMemoryStore::default();
    for (operation, correlation_id, result) in [
        ("KillSwitch", "corr-audit-page-1", "ACCEPTED"),
        ("RuntimeOverride", "corr-audit-page-2", "DENIED"),
        ("KillSwitch", "corr-audit-page-3", "ACCEPTED"),
    ] {
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: "admin-page-test".into(),
                operation: operation.into(),
                request_fingerprint: Some(format!("fp-{correlation_id}")),
                correlation_id: Some(correlation_id.into()),
                result: result.into(),
                created_at: None,
            })
            .await
            .expect("record audit page event");
    }

    let first_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("first page");
    assert_eq!(first_page.len(), 2);
    assert_eq!(
        first_page
            .iter()
            .map(|event| event.correlation_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("corr-audit-page-2"), Some("corr-audit-page-3")]
    );

    let older_page = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 2,
            before_audit_id: first_page[0].audit_id,
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("older page");
    assert_eq!(older_page.len(), 1);
    assert_eq!(
        older_page[0].correlation_id.as_deref(),
        Some("corr-audit-page-1")
    );

    let filtered = store
        .list_admin_audit_events(&AdminAuditQuery {
            limit: 10,
            operation: Some("KillSwitch".into()),
            result: Some("ACCEPTED".into()),
            correlation_id: Some("corr-audit-page-3".into()),
            principal_subject: Some("admin-page-test".into()),
            ..AdminAuditQuery::default()
        })
        .await
        .expect("filtered page");
    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].correlation_id.as_deref(),
        Some("corr-audit-page-3")
    );
}

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
