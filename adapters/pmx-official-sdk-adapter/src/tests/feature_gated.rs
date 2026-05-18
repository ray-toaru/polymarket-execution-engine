#[cfg(any(
    feature = "sdk-typecheck",
    feature = "authenticated-smoke",
    feature = "sign-only-dry-run"
))]
use super::*;

#[cfg(feature = "sdk-typecheck")]
#[test]
fn sdk_error_normalization_covers_validation() {
    let err = SdkError::validation("bad builder");
    let normalized = normalize_sdk_error(&err);
    assert_eq!(
        normalized.category,
        OfficialSdkErrorCategory::ValidationFailed
    );
    assert!(!normalized.retryable);
}

#[cfg(feature = "sdk-typecheck")]
#[test]
fn geoblock_status_maps_to_core_status() {
    assert_eq!(geoblock_status_from_sdk(true), GeoblockStatus::Blocked);
    assert_eq!(geoblock_status_from_sdk(false), GeoblockStatus::Allowed);
}

#[cfg(feature = "sdk-typecheck")]
#[test]
fn sdk_error_normalization_covers_status_codes() {
    let err = SdkError::status(
        polymarket_client_sdk_v2::error::StatusCode::TOO_MANY_REQUESTS,
        polymarket_client_sdk_v2::error::Method::GET,
        "/orders".into(),
        "rate limited",
    );
    let normalized = normalize_sdk_error(&err);
    assert_eq!(normalized.category, OfficialSdkErrorCategory::RemoteUnknown);
    assert!(normalized.retryable);
    assert_eq!(normalized.http_status, Some(429));
}

#[cfg(feature = "sdk-typecheck")]
#[test]
fn gateway_error_conversion_preserves_remote_unknown() {
    let normalized = OfficialSdkNormalizedError {
        category: OfficialSdkErrorCategory::WebSocketFailed,
        retryable: true,
        message: "timeout".into(),
        http_status: None,
        geoblock_country: None,
        geoblock_region: None,
    };
    assert_eq!(
        gateway_error_from_normalized_sdk_error(&normalized),
        GatewayError::RemoteUnknown("timeout".into())
    );
}

#[cfg(feature = "authenticated-smoke")]
#[tokio::test(flavor = "current_thread")]
async fn authenticated_non_trading_smoke_executes_when_enabled() {
    if !env_flag(ENV_RUN_AUTHENTICATED_SMOKE) || !env_present(PRIVATE_KEY_VAR_NAME) {
        eprintln!("skipping authenticated non-trading smoke test; env gate not enabled");
        return;
    }
    let config = OfficialSdkAdapterConfig {
        allow_authenticated_non_trading_smoke: true,
        ..OfficialSdkAdapterConfig::default()
    };
    let report = run_authenticated_non_trading_sdk_smoke(&config)
        .await
        .expect("authenticated non-trading smoke should succeed when env is explicitly enabled");
    assert!(!report.ok_status.is_empty());
    assert!(report.server_time > 0);
    assert!(report.credential_snapshot.has_l1_private_key);
}

#[cfg(feature = "sign-only-dry-run")]
#[tokio::test(flavor = "current_thread")]
async fn sign_only_dry_run_executes_when_enabled() {
    if !env_flag(ENV_RUN_SIGN_ONLY_DRY_RUN)
        || !env_flag(ENV_ALLOW_SIGN_ONLY_DRY_RUN)
        || !env_present(PRIVATE_KEY_VAR_NAME)
    {
        eprintln!("skipping sign-only dry-run test; env gate not enabled");
        return;
    }
    let config = OfficialSdkAdapterConfig {
        allow_sign_only_dry_run: true,
        ..OfficialSdkAdapterConfig::default()
    };
    let token_id = match std::env::var("PMX_SIGN_ONLY_TOKEN_ID") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => discover_active_token_id(&config)
            .await
            .expect("sign-only dry-run requires a discoverable live token_id"),
    };
    let receipt = run_sign_only_dry_run(
        &config,
        SignOnlyDryRunRequest {
            account_id: AccountId("acct-a".into()),
            execution_id: ExecutionId("exec-sign-only".into()),
            plan_hash: HashValue("plan-sign-only".into()),
            token_id,
            side: "BUY".into(),
            size: "1".into(),
            limit_price: "0.50".into(),
        },
    )
    .await
    .expect("sign-only dry-run should succeed when env is explicitly enabled");
    assert!(!receipt.posted);
    assert!(receipt.signed_order_ref.starts_with("sign-only:"));
}
