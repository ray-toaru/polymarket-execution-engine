use super::*;

#[test]
fn order_lifecycle_divergence_maps_remote_unknown_open_and_missing() {
    let open = classify_order_lifecycle_divergence(
        &OrderLifecycleState::RemoteUnknown,
        RemoteOrderObservation::Open,
    );
    assert_eq!(
        open.kind,
        OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteOpen
    );
    assert_eq!(open.event, Some(OrderEventKind::ReconcileOpen));
    assert!(!open.operator_required);
    assert!(open.no_remote_side_effect);

    let first_missing = classify_order_lifecycle_divergence(
        &OrderLifecycleState::RemoteUnknown,
        RemoteOrderObservation::Missing,
    );
    assert_eq!(first_missing.event, Some(OrderEventKind::ReconcileMissing));
    assert!(!first_missing.operator_required);

    let second_missing = classify_order_lifecycle_divergence(
        &OrderLifecycleState::PartialRemoteUnknown,
        RemoteOrderObservation::Missing,
    );
    assert_eq!(second_missing.event, Some(OrderEventKind::ReconcileMissing));
    assert!(second_missing.operator_required);

    let still_unknown = classify_order_lifecycle_divergence(
        &OrderLifecycleState::RemoteUnknown,
        RemoteOrderObservation::Unknown,
    );
    assert_eq!(still_unknown.event, Some(OrderEventKind::ReconcileUnknown));
    assert!(still_unknown.operator_required);
    assert_eq!(
        transition_order_state(
            OrderLifecycleState::RemoteUnknown,
            OrderEventKind::ReconcileUnknown
        )
        .expect("reconcile unknown transition"),
        OrderLifecycleState::RemoteUnknown
    );
}

#[test]
fn order_lifecycle_divergence_escalates_terminal_remote_mismatch() {
    let divergence = classify_order_lifecycle_divergence(
        &OrderLifecycleState::Filled,
        RemoteOrderObservation::Open,
    );
    assert_eq!(
        divergence.kind,
        OrderLifecycleDivergenceKind::TerminalLocalRemoteMismatch
    );
    assert!(divergence.event.is_none());
    assert!(divergence.operator_required);
    assert!(divergence.no_remote_side_effect);
}
