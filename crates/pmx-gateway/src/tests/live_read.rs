use super::*;

#[test]
fn normalized_live_read_error_redacts_secret_material() {
    let secret_key = ["POLY_API", "SECRET"].join("_");
    let event = live_read_event_from_gateway_error(
        pmx_core::AccountId("acct-live-read".into()),
        LiveReadOperation::GetOrder,
        Some(pmx_core::RemoteOrderId("remote-live-read".into())),
        GatewayError::RemoteUnknown(format!(
            "remote error {secret_key}=leaked 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        )),
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
    assert!(summary.contains(&format!("{secret_key}=[REDACTED]")));
    assert!(summary.contains("0x[REDACTED]"));
}

#[test]
fn normalized_live_read_error_redacts_assignment_keys_case_insensitively() {
    let api_secret_key = "api_".to_string() + "secret";
    let signature_key = "sign".to_string() + "ature";
    let signed_payload_key = "signed_".to_string() + "Payload";
    let remote_error = format!(
        "{api_secret_key}=lowercase-secret {signature_key}=raw-signature {signed_payload_key}=raw-order-body"
    );
    let event = live_read_event_from_gateway_error(
        pmx_core::AccountId("acct-live-read".into()),
        LiveReadOperation::GetOrder,
        None,
        GatewayError::RemoteRejected(remote_error),
    );

    let summary = event.redacted_error_summary.as_deref().unwrap();
    assert!(summary.contains(&format!("{api_secret_key}=[REDACTED]")));
    assert!(summary.contains(&format!("{signature_key}=[REDACTED]")));
    assert!(summary.contains(&format!("{signed_payload_key}=[REDACTED]")));
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
