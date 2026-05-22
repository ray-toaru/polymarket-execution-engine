use super::super::*;

#[tokio::test]
async fn service_records_standard_sign_only_construction_without_raw_payload() {
    let store = InMemoryStore::default();
    let service = ExecutorService::new(store.clone());
    let plan_hash = seed_test_plan(&store, "exec-sdk-standard", "acct-sdk-standard").await;

    let receipt = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard".into(),
            account_id: "acct-sdk-standard".into(),
            plan_hash: plan_hash.clone(),
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
    let plan_hash = seed_test_plan(
        &store,
        "exec-sdk-standard-derived",
        "acct-sdk-standard-derived",
    )
    .await;

    let first = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard-derived".into(),
            account_id: "acct-sdk-standard-derived".into(),
            plan_hash: plan_hash.clone(),
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
            plan_hash: plan_hash.clone(),
            signed_order_ref: None,
            signed_order_digest: None,
            no_remote_side_effect: true,
        })
        .await
        .expect("replay derived standard sign-only construction");

    assert!(first.no_remote_side_effect);
    assert!(first.signed_order_ref.starts_with(&format!(
        "sign-only:exec-sdk-standard-derived:{plan_hash}:digest-"
    )));
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
    let plan_hash = seed_test_plan(
        &store,
        "exec-sdk-standard-bad-digest",
        "acct-sdk-standard-bad-digest",
    )
    .await;

    let err = service
        .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
            execution_id: "exec-sdk-standard-bad-digest".into(),
            account_id: "acct-sdk-standard-bad-digest".into(),
            plan_hash,
            signed_order_ref: Some("sign-only:digest-ref".into()),
            signed_order_digest: Some("not-a-sha256".into()),
            no_remote_side_effect: true,
        })
        .await
        .expect_err("malformed digest must be rejected");

    assert!(matches!(err, ServiceError::BadRequest(_)));
}
