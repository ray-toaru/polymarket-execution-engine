use super::*;

#[test]
fn cannot_confirm_cancel_without_pending_cancel() {
    let err = transition_order_state(OrderLifecycleState::Posted, OrderEventKind::CancelConfirmed)
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidTransition { .. }));
}

#[test]
fn cancel_confirmation_requires_remote_acceptance() {
    let s1 = transition_order_state(OrderLifecycleState::Posted, OrderEventKind::CancelRequested)
        .unwrap();
    let s2 = transition_order_state(s1, OrderEventKind::CancelRemoteAccepted).unwrap();
    let s3 = transition_order_state(s2, OrderEventKind::CancelConfirmed).unwrap();
    assert_eq!(s3, OrderLifecycleState::CancelConfirmed);
}

#[test]
fn cancel_state_tracks_lifecycle_pending_and_terminal_states() {
    assert_eq!(
        cancel_state_from_lifecycle(&OrderLifecycleState::CancelRequested),
        CancelState::Requested
    );
    assert_eq!(
        cancel_state_from_lifecycle(&OrderLifecycleState::CancelRemoteAccepted),
        CancelState::RemoteAccepted
    );
    assert_eq!(
        cancel_state_from_lifecycle(&OrderLifecycleState::CancelConfirmed),
        CancelState::ConfirmedCanceled
    );
}

#[test]
fn remote_unknown_states_require_reconcile() {
    let state = transition_order_state(
        OrderLifecycleState::PostRequested,
        OrderEventKind::RemoteUnknown,
    )
    .unwrap();
    assert!(lifecycle_requires_reconcile(&state));
    assert_eq!(
        cancel_state_from_lifecycle(&state),
        CancelState::RemoteUnknown
    );
}

#[test]
fn sign_only_lifecycle_never_models_remote_post() {
    let s1 = transition_sign_only_lifecycle(
        SignOnlyLifecycleState::Planned,
        SignOnlyLifecycleEventKind::PrepareReservation,
    )
    .unwrap();
    let s2 =
        transition_sign_only_lifecycle(s1, SignOnlyLifecycleEventKind::RequestSigning).unwrap();
    let s3 =
        transition_sign_only_lifecycle(s2, SignOnlyLifecycleEventKind::SignedWithoutPost).unwrap();
    assert_eq!(s3, SignOnlyLifecycleState::SignedDryRun);
    let record = SignOnlyLifecycleRecord {
        execution_id: ExecutionId("exec-sign-only".into()),
        account_id: AccountId("acct-1".into()),
        state: s3,
        event: SignOnlyLifecycleEventKind::SignedWithoutPost,
        client_event_id: None,
        signed_order_ref: Some("sign-only:exec:hash:sig-abcd".into()),
        no_remote_side_effect: true,
        event_id: None,
        created_at: None,
    };
    assert!(!sign_only_lifecycle_has_remote_side_effect(&record));
}

#[test]
fn sign_only_lifecycle_rejects_direct_sign_without_reservation() {
    let err = transition_sign_only_lifecycle(
        SignOnlyLifecycleState::Planned,
        SignOnlyLifecycleEventKind::SignedWithoutPost,
    )
    .expect_err("direct sign-only completion must be invalid");
    assert!(matches!(err, CoreError::InvalidSignOnlyTransition { .. }));
}

#[test]
fn reconcile_action_tracks_remote_unknown_and_partial_unknown() {
    assert_eq!(
        reconcile_action_for_lifecycle(&OrderLifecycleState::RemoteUnknown),
        ReconcileAction::QueryRemoteOpenOrder
    );
    assert_eq!(
        reconcile_action_for_lifecycle(&OrderLifecycleState::PartialRemoteUnknown),
        ReconcileAction::ConfirmMissingOrEscalate
    );
    assert_eq!(
        reconcile_action_for_lifecycle(&OrderLifecycleState::Posted),
        ReconcileAction::Noop
    );
}
