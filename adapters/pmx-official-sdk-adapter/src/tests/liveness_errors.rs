use super::*;

#[test]
fn liveness_requires_reconcile_when_remote_unknown_exists() {
    let disposition = assess_sdk_liveness(&OfficialSdkLivenessSnapshot {
        websocket_connected: true,
        heartbeat_expected: true,
        heartbeats_active: true,
        geoblock_status: GeoblockStatus::Allowed,
        remote_unknown_orders: 2,
    });
    assert_eq!(
        disposition,
        OfficialSdkReconcileDisposition::ReconcileRequired
    );
}

#[test]
fn liveness_geoblock_blocks_first() {
    let disposition = assess_sdk_liveness(&OfficialSdkLivenessSnapshot {
        websocket_connected: true,
        heartbeat_expected: false,
        heartbeats_active: false,
        geoblock_status: GeoblockStatus::Blocked,
        remote_unknown_orders: 10,
    });
    assert_eq!(disposition, OfficialSdkReconcileDisposition::Geoblocked);
}

#[test]
fn redacts_named_secret_assignments() {
    let message = "request failed POLY_API_SECRET=super-secret POLY_API_PASSPHRASE=pass";
    let redacted = redact_sensitive_text(message);
    assert!(redacted.contains("POLY_API_SECRET=[REDACTED]"));
    assert!(redacted.contains("POLY_API_PASSPHRASE=[REDACTED]"));
    assert!(!redacted.contains("super-secret"));
    assert!(!redacted.contains("pass"));
}

#[test]
fn redacts_private_key_like_hex_tokens() {
    let key = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let redacted = redact_sensitive_text(&format!("sdk error included {key}"));
    assert!(redacted.contains("0x[REDACTED]"));
    assert!(!redacted.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
}

#[test]
fn gateway_error_conversion_redacts_sensitive_message() {
    let normalized = OfficialSdkNormalizedError {
        category: OfficialSdkErrorCategory::RemoteRejected,
        retryable: false,
        message: "POLY_API_SECRET=leaked-secret".into(),
        http_status: Some(400),
        geoblock_country: None,
        geoblock_region: None,
    };
    assert_eq!(
        gateway_error_from_normalized_sdk_error(&normalized),
        GatewayError::RemoteRejected("POLY_API_SECRET=[REDACTED]".into())
    );
}

#[test]
fn normalized_error_redaction_covers_remote_unknown_messages() {
    let normalized = OfficialSdkNormalizedError {
        category: OfficialSdkErrorCategory::RemoteUnknown,
        retryable: true,
        message: "timeout POLY_API_SECRET=leaked-secret".into(),
        http_status: Some(503),
        geoblock_country: None,
        geoblock_region: None,
    };
    let redacted = redact_normalized_error(&normalized);
    assert!(!redacted.message.contains("leaked-secret"));
    assert_eq!(
        gateway_error_from_normalized_sdk_error(&redacted),
        GatewayError::RemoteUnknown("timeout POLY_API_SECRET=[REDACTED]".into())
    );
}
