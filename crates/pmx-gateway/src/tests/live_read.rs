use super::*;

#[test]
fn normalized_live_read_error_redacts_secret_material() {
    let event = LiveReadNormalizedEvent::from_gateway_error(
        pmx_core::AccountId("acct-live-read".into()),
        LiveReadOperation::GetOrder,
        Some(pmx_core::RemoteOrderId("remote-live-read".into())),
        GatewayError::RemoteUnknown(
            "remote error POLY_API_SECRET=leaked 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
        ),
    );

    assert_eq!(event.outcome, LiveReadOutcome::RemoteUnknown);
    assert_eq!(
        event.error_category,
        Some(LiveReadErrorCategory::RemoteUnknown)
    );
    assert!(event.no_trading_side_effect);
    assert!(event.redacted_fields.contains(&"raw_error".to_string()));
    let summary = event.redacted_error_summary.as_deref().unwrap();
    assert!(!summary.contains("leaked"));
    assert!(!summary.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    assert!(summary.contains("POLY_API_SECRET=[REDACTED]"));
    assert!(summary.contains("0x[REDACTED]"));
}

#[test]
fn normalized_live_read_error_redacts_assignment_keys_case_insensitively() {
    let event = LiveReadNormalizedEvent::from_gateway_error(
        pmx_core::AccountId("acct-live-read".into()),
        LiveReadOperation::GetOrder,
        None,
        GatewayError::RemoteRejected(
            "api_secret=lowercase-secret signature=raw-signature signed_Payload=raw-order-body"
                .into(),
        ),
    );

    let summary = event.redacted_error_summary.as_deref().unwrap();
    assert!(summary.contains("api_secret=[REDACTED]"));
    assert!(summary.contains("signature=[REDACTED]"));
    assert!(summary.contains("signed_Payload=[REDACTED]"));
    assert!(!summary.contains("lowercase-secret"));
    assert!(!summary.contains("raw-signature"));
    assert!(!summary.contains("raw-order-body"));
}

#[test]
fn normalized_live_read_order_observation_is_allowlisted_and_read_only() {
    let remote_order = RemoteOrder {
        remote_order_id: pmx_core::RemoteOrderId("remote-live-read-open".into()),
        account_id: pmx_core::AccountId("acct-live-read".into()),
        state: "OPEN".into(),
    };

    let event = LiveReadNormalizedEvent::observed_order(LiveReadOperation::GetOrder, remote_order);

    assert_eq!(event.operation, LiveReadOperation::GetOrder);
    assert_eq!(event.outcome, LiveReadOutcome::Observed);
    assert_eq!(
        event.remote_order_id,
        Some(pmx_core::RemoteOrderId("remote-live-read-open".into()))
    );
    assert_eq!(event.remote_state.as_deref(), Some("OPEN"));
    assert!(event.no_trading_side_effect);
    assert!(
        event
            .redacted_fields
            .contains(&"raw_remote_payload".to_string())
    );
}
