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
