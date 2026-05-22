use super::*;

#[tokio::test]
async fn same_request_replay_is_persisted() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    let action = store
        .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
        .await
        .expect("begin idempotency");
    let IdempotencyAction::Proceed {
        submit_attempt,
        owner_token,
    } = action
    else {
        panic!("first request must proceed");
    };
    assert_eq!(submit_attempt, 1);
    assert!(owner_token.starts_with("owner-"));
    store
        .finish_submit_attempt(FinishSubmitAttempt {
            account_id: &account,
            execution_id: &execution,
            idempotency_key: "idem-1",
            request_fingerprint: "req-1",
            owner_token: &owner_token,
            response_fingerprint: "resp-1",
            response_json: r#"{"status":"accepted"}"#,
        })
        .await
        .expect("finish idempotency");
    let replay = store
        .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
        .await
        .expect("replay idempotency");
    assert!(matches!(
        replay,
        IdempotencyAction::ReplayStoredResponse { .. }
    ));
}

#[tokio::test]
async fn fingerprint_mismatch_is_conflict() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    store
        .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
        .await
        .expect("begin idempotency");
    let conflict = store
        .begin_submit_attempt(&account, &execution, "idem-1", "req-2")
        .await
        .expect("conflict result");
    assert_eq!(conflict, IdempotencyAction::Conflict);
}

#[tokio::test]
async fn in_progress_replay_does_not_return_proceed() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    let first = store
        .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
        .await
        .expect("first begin");
    assert!(matches!(first, IdempotencyAction::Proceed { .. }));
    let second = store
        .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
        .await
        .expect("second begin");
    assert!(matches!(second, IdempotencyAction::InProgress { .. }));
}

#[tokio::test]
async fn finish_requires_current_owner_token() {
    let Some(store) = test_store().await else {
        return;
    };
    let account = unique("acct");
    let execution = unique("exec");
    seed_execution_plan(&store, &account, &execution).await;
    let first = store
        .begin_submit_attempt(&account, &execution, "idem-owner", "req-owner")
        .await
        .expect("first begin");
    let IdempotencyAction::Proceed { owner_token, .. } = first else {
        panic!("first begin must proceed");
    };
    let wrong_owner = store
        .finish_submit_attempt(FinishSubmitAttempt {
            account_id: &account,
            execution_id: &execution,
            idempotency_key: "idem-owner",
            request_fingerprint: "req-owner",
            owner_token: "owner-stale",
            response_fingerprint: "resp-owner",
            response_json: r#"{"status":"accepted"}"#,
        })
        .await
        .expect_err("wrong owner cannot finish");
    assert!(matches!(wrong_owner, StoreError::Conflict(_)));
    store
        .finish_submit_attempt(FinishSubmitAttempt {
            account_id: &account,
            execution_id: &execution,
            idempotency_key: "idem-owner",
            request_fingerprint: "req-owner",
            owner_token: &owner_token,
            response_fingerprint: "resp-owner",
            response_json: r#"{"status":"accepted"}"#,
        })
        .await
        .expect("current owner can finish");
}
