use super::*;
use crate::{advisory_lock_key, submit_status_str};
use pmx_core::SubmitStatus;

#[test]
fn idempotency_identity_is_documented_in_trait() {
    let action = IdempotencyAction::Proceed {
        submit_attempt: 1,
        owner_token: "owner".into(),
    };
    assert_eq!(
        action,
        IdempotencyAction::Proceed {
            submit_attempt: 1,
            owner_token: "owner".into(),
        }
    );
}

#[test]
fn advisory_lock_key_is_deterministic_and_scoped() {
    let a = advisory_lock_key("submit", "acct-1", "exec-1");
    let b = advisory_lock_key("submit", "acct-1", "exec-1");
    let c = advisory_lock_key("submit", "acct-1", "exec-2");
    let d = advisory_lock_key("reservation", "acct-1", "exec-1");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
}

#[test]
fn maps_plan_status_for_db() {
    let status = submit_status_str(&SubmitStatus::RemoteUnknown);
    assert_eq!(status, "REMOTE_UNKNOWN");
}

#[test]
fn runtime_state_query_key_includes_sorted_required_capabilities() {
    let base = RuntimeStateQuery {
        account_id: "acct".into(),
        condition_id: "cond".into(),
        collateral_profile_id: None,
        required_capabilities: vec!["reconcile".into(), "heartbeat".into()],
    };
    let same_set = RuntimeStateQuery {
        required_capabilities: vec!["heartbeat".into(), "reconcile".into()],
        ..base.clone()
    };
    let different_set = RuntimeStateQuery {
        required_capabilities: vec!["heartbeat".into()],
        ..base.clone()
    };
    assert_eq!(base.key(), same_set.key());
    assert_ne!(base.key(), different_set.key());
}

#[tokio::test]
async fn in_memory_same_request_without_response_is_in_progress() {
    let store = InMemoryStore::default();
    let first = store
        .begin_submit_attempt("acct", "exec", "idem", "req")
        .await
        .expect("first begin");
    assert!(matches!(first, IdempotencyAction::Proceed { .. }));
    let second = store
        .begin_submit_attempt("acct", "exec", "idem", "req")
        .await
        .expect("second begin");
    assert!(matches!(second, IdempotencyAction::InProgress { .. }));
}

#[tokio::test]
async fn in_memory_finish_requires_current_owner_token() {
    let store = InMemoryStore::default();
    let first = store
        .begin_submit_attempt("acct", "exec-owner", "idem", "req")
        .await
        .expect("first begin");
    let IdempotencyAction::Proceed { owner_token, .. } = first else {
        panic!("first begin must proceed");
    };
    let wrong_owner = store
        .finish_submit_attempt(FinishSubmitAttempt {
            account_id: "acct",
            execution_id: "exec-owner",
            idempotency_key: "idem",
            request_fingerprint: "req",
            owner_token: "owner-stale",
            response_fingerprint: "resp",
            response_json: r#"{"status":"blocked"}"#,
        })
        .await
        .expect_err("wrong owner cannot finish");
    assert!(matches!(wrong_owner, StoreError::Conflict(_)));
    store
        .finish_submit_attempt(FinishSubmitAttempt {
            account_id: "acct",
            execution_id: "exec-owner",
            idempotency_key: "idem",
            request_fingerprint: "req",
            owner_token: &owner_token,
            response_fingerprint: "resp",
            response_json: r#"{"status":"blocked"}"#,
        })
        .await
        .expect("current owner can finish");
}
