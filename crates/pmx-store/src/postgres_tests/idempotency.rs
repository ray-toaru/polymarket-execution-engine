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
    assert_eq!(
        action,
        IdempotencyAction::Proceed {
            submit_attempt: 1,
            owner_token: format!("owner-{account}-{execution}-1"),
        }
    );
    store
        .finish_submit_attempt(
            &account,
            &execution,
            "idem-1",
            "req-1",
            "resp-1",
            r#"{"status":"accepted"}"#,
        )
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
