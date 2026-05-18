use super::*;
use serde::Serialize;

fn base_intent(side: Side, quantity: QuantityIntent) -> TradeIntent {
    TradeIntent {
        client_intent_id: "intent-1".into(),
        account_id: AccountId("acct-1".into()),
        market: MarketRef {
            condition_id: ConditionId("cond-1".into()),
            slug: None,
            is_sports: false,
        },
        token_id: TokenId("token-1".into()),
        side,
        quantity,
        limit_price: DecimalString("0.51".into()),
        time_in_force: TimeInForce::Gtc,
        collateral_profile_id: None,
    }
}

#[test]
fn decimal_rejects_scientific_padding_and_trailing_dot() {
    for bad in ["", " 1", "1 ", "1e-3", "+1", "-1", ".5", "1.", "00.1"] {
        assert!(
            validate_decimal_string(bad).is_err(),
            "{bad} should be invalid"
        );
    }
    assert!(validate_decimal_string("0.5").is_ok());
}

#[test]
fn limit_price_is_executor_authoritative() {
    for bad in ["0", "0.0", "1.01", "2", "1.0001"] {
        let mut intent = base_intent(
            Side::Buy,
            QuantityIntent {
                max_notional: Some(DecimalString("10".into())),
                max_shares: None,
            },
        );
        intent.limit_price = DecimalString(bad.into());
        assert!(matches!(
            normalize_intent(intent),
            Err(CoreError::InvalidLimitPrice(_))
        ));
    }
    let mut intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    );
    intent.limit_price = DecimalString("1".into());
    assert!(normalize_intent(intent).is_ok());
}

#[test]
fn quantity_must_be_positive() {
    let intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("0".into())),
            max_shares: None,
        },
    );
    assert!(matches!(
        normalize_intent(intent),
        Err(CoreError::InvalidQuantity(_))
    ));
}

#[test]
fn quantity_requires_exactly_one_bound() {
    let intent = base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: None,
            max_shares: None,
        },
    );
    assert_eq!(
        normalize_intent(intent).unwrap_err(),
        CoreError::QuantityBoundCardinality
    );
}

#[test]
fn buy_notional_canonicalizes_to_quote_bound() {
    let n = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    assert!(matches!(
        n.quantity_bound,
        QuantityBound::WorstCaseQuoteNotional(_)
    ));
}

#[test]
fn sell_shares_canonicalizes_to_base_bound() {
    let n = normalize_intent(base_intent(
        Side::Sell,
        QuantityIntent {
            max_notional: None,
            max_shares: Some(DecimalString("7".into())),
        },
    ))
    .unwrap();
    assert!(matches!(
        n.quantity_bound,
        QuantityBound::WorstCaseBaseShares(_)
    ));
}

#[test]
fn unsupported_cross_quantity_is_explicit() {
    let n = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: None,
            max_shares: Some(DecimalString("7".into())),
        },
    ))
    .unwrap();
    assert!(matches!(n.quantity_bound, QuantityBound::Unsupported(_)));
}

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
fn canonical_json_hash_is_key_order_independent() {
    #[derive(Serialize)]
    struct Left {
        b: u8,
        a: u8,
    }
    #[derive(Serialize)]
    struct Right {
        a: u8,
        b: u8,
    }

    let left = canonical_json_sha256(&Left { b: 2, a: 1 }).unwrap();
    let right = canonical_json_sha256(&Right { a: 1, b: 2 }).unwrap();
    assert_eq!(left, right);
    assert_eq!(left.0.len(), 64);
}

#[test]
fn normalized_intent_hash_is_content_derived() {
    let first = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    let second = normalize_intent(base_intent(
        Side::Buy,
        QuantityIntent {
            max_notional: Some(DecimalString("10".into())),
            max_shares: None,
        },
    ))
    .unwrap();
    assert_eq!(first.intent_hash, second.intent_hash);
    assert!(first.normalized_intent_id.starts_with("norm-"));
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
