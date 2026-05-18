use super::*;

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
async fn service_records_standard_sign_only_construction_without_raw_payload() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    seed_test_plan(&store, "exec-sdk-standard", "acct-sdk-standard").await;

    let receipt = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard".into(),
            account_id: "acct-sdk-standard".into(),
            plan_hash: "hash-exec-sdk-standard".into(),
            signed_order_ref: Some("sign-only:digest-ref".into()),
            signed_order_digest: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            ),
            no_remote_side_effect: true,
        })
        .await
        .expect("record standard sign-only construction");

    assert!(receipt.no_remote_side_effect);
    assert_eq!(
        receipt.signed_order_digest.as_deref(),
        Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    );
    assert_eq!(receipt.lifecycle_records.len(), 3);
    assert_eq!(
        receipt.lifecycle_records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
    assert_eq!(
        receipt
            .lifecycle_records
            .last()
            .unwrap()
            .signed_order_ref
            .as_deref(),
        Some("sign-only:digest-ref")
    );
}

#[tokio::test]
async fn service_derives_standard_sign_only_ref_and_digest_by_default() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    seed_test_plan(
        &store,
        "exec-sdk-standard-derived",
        "acct-sdk-standard-derived",
    )
    .await;

    let first = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard-derived".into(),
            account_id: "acct-sdk-standard-derived".into(),
            plan_hash: "hash-exec-sdk-standard-derived".into(),
            signed_order_ref: None,
            signed_order_digest: None,
            no_remote_side_effect: true,
        })
        .await
        .expect("derive standard sign-only construction");
    let replay = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard-derived".into(),
            account_id: "acct-sdk-standard-derived".into(),
            plan_hash: "hash-exec-sdk-standard-derived".into(),
            signed_order_ref: None,
            signed_order_digest: None,
            no_remote_side_effect: true,
        })
        .await
        .expect("replay derived standard sign-only construction");

    assert!(first.no_remote_side_effect);
    assert!(
        first.signed_order_ref.starts_with(
            "sign-only:exec-sdk-standard-derived:hash-exec-sdk-standard-derived:digest-"
        )
    );
    assert_eq!(first.signed_order_digest.as_ref().unwrap().len(), 64);
    assert_eq!(first.signed_order_ref, replay.signed_order_ref);
    assert_eq!(first.signed_order_digest, replay.signed_order_digest);
    assert_eq!(first.lifecycle_records.len(), 3);
    assert_eq!(replay.lifecycle_records.len(), 3);
}

#[tokio::test]
async fn service_rejects_malformed_standard_sign_only_digest() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    seed_test_plan(
        &store,
        "exec-sdk-standard-bad-digest",
        "acct-sdk-standard-bad-digest",
    )
    .await;

    let err = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard-bad-digest".into(),
            account_id: "acct-sdk-standard-bad-digest".into(),
            plan_hash: "hash-exec-sdk-standard-bad-digest".into(),
            signed_order_ref: Some("sign-only:digest-ref".into()),
            signed_order_digest: Some("not-a-sha256".into()),
            no_remote_side_effect: true,
        })
        .await
        .expect_err("malformed digest must be rejected");

    assert!(matches!(err, ServiceError::BadRequest(_)));
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
