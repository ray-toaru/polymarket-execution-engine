use super::*;
use pmx_core::{AccountId, ExecutionId, GeoblockStatus, HashValue, SignOnlyLifecycleState};
use pmx_gateway::GatewayError;

#[cfg(feature = "sdk-typecheck")]
use polymarket_client_sdk_v2::error::Error as SdkError;

fn empty_credentials() -> AdapterCredentialSnapshot {
    AdapterCredentialSnapshot {
        has_l1_private_key: false,
        has_l2_api_key: false,
        has_l2_api_secret: false,
        has_l2_passphrase: false,
    }
}

fn l1_credentials() -> AdapterCredentialSnapshot {
    AdapterCredentialSnapshot {
        has_l1_private_key: true,
        has_l2_api_key: false,
        has_l2_api_secret: false,
        has_l2_passphrase: false,
    }
}

fn sample_plan_limit() -> OfficialSdkPlanOrder {
    OfficialSdkPlanOrder {
        execution_id: ExecutionId("exec-1".into()),
        account_id: AccountId("acct-1".into()),
        token_id: "123".into(),
        side: "buy".into(),
        order_kind: "limit".into(),
        limit_price: Some("0.55".into()),
        size: Some("10".into()),
        amount: None,
        time_in_force: Some("gtc".into()),
        post_only: Some(false),
        builder_attribution: None,
        fee_rate_bps: None,
        funder: None,
        signer: None,
        signature_type: None,
    }
}

#[test]
fn default_config_cannot_live_submit() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(!config.allow_authenticated_non_trading_smoke);
    assert!(!config.allow_sign_only_dry_run);
    assert!(!config.allow_live_submit);
    assert!(config.require_kill_switch_open_for_live_submit);
    assert!(config.require_repository_reservation_for_live_submit);
    assert!(config.require_reconcile_worker_for_live_submit);
}

#[test]
fn standard_sign_only_profile_is_non_posting_v2_pusd() {
    let profile = OfficialSdkStandardSignOnlyProfile::default();
    validate_standard_sign_only_profile(&profile).expect("standard sign-only profile");
    assert_eq!(profile.clob_host, CLOB_V2_HOST);
    assert_eq!(profile.collateral_symbol, "pUSD");
    assert_eq!(profile.signing_protocol, "CLOB_V2");
    assert!(profile.uses_deposit_wallet_order_path);
    assert!(!profile.exposes_raw_signed_order);
    assert!(!profile.may_post_order);
    assert!(!profile.may_cancel_order);
}

#[test]
fn standard_sign_only_plan_is_default_sdk_construct_path_without_raw_payload() {
    let plan = standard_sign_only_default_plan_for_order(&sample_plan_limit())
        .expect("standard sign-only plan");
    assert_eq!(plan.signed_order_ref_namespace, "sign-only");
    assert_eq!(plan.mapping.order_kind, "LIMIT");
    assert_eq!(plan.mapping.time_in_force.as_deref(), Some("GTC"));
    assert_eq!(plan.profile.clob_host, CLOB_V2_HOST);
    assert_eq!(plan.profile.collateral_symbol, CLOB_V2_COLLATERAL_SYMBOL);
    assert!(plan.profile.uses_deposit_wallet_order_path);
    assert!(!plan.exposes_raw_signed_order);
    assert!(!plan.may_post_order);
    assert!(!plan.may_cancel_order);
}

#[test]
fn standard_sign_only_construction_emits_only_digest_ref_and_lifecycle() {
    let construction = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-standard".into()),
    )
    .expect("standard sign-only construction");
    assert!(construction.no_remote_side_effect);
    assert!(!construction.raw_signed_order_exposed);
    assert!(!construction.signed_order_digest.is_empty());
    assert!(
        construction
            .signed_order_ref
            .starts_with("sign-only:exec-1:plan-hash-standard:digest-")
    );
    assert_eq!(construction.lifecycle_records.len(), 3);
    assert_eq!(
        construction.lifecycle_records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
    assert_eq!(
        construction
            .lifecycle_records
            .last()
            .unwrap()
            .signed_order_ref
            .as_deref(),
        Some(construction.signed_order_ref.as_str())
    );
}

#[test]
fn standard_sign_only_construction_ref_is_stable_for_same_mapping() {
    let first = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-stable".into()),
    )
    .expect("first construction");
    let second = standard_sign_only_construction_for_order(
        &sample_plan_limit(),
        HashValue("plan-hash-stable".into()),
    )
    .expect("second construction");
    assert_eq!(first.signed_order_ref, second.signed_order_ref);
    assert_eq!(first.signed_order_digest, second.signed_order_digest);
}

#[test]
fn standard_sign_only_plan_rejects_profile_that_can_post_or_expose_raw_order() {
    let profile = OfficialSdkStandardSignOnlyProfile {
        exposes_raw_signed_order: true,
        ..OfficialSdkStandardSignOnlyProfile::default()
    };
    let err = standard_sign_only_plan_for_order(profile, &sample_plan_limit())
        .expect_err("raw order exposure must be rejected");
    assert!(
        err.to_string()
            .contains("must not expose raw signed orders")
    );

    let profile = OfficialSdkStandardSignOnlyProfile {
        may_post_order: true,
        ..OfficialSdkStandardSignOnlyProfile::default()
    };
    let err = standard_sign_only_plan_for_order(profile, &sample_plan_limit())
        .expect_err("posting profile must be rejected");
    assert!(err.to_string().contains("post orders"));
}

#[test]
fn read_only_smoke_ignores_ambient_credentials_but_must_remain_unauthenticated() {
    validate_read_only_smoke_environment(&empty_credentials()).expect("empty credentials allowed");
    validate_read_only_smoke_environment(&l1_credentials())
        .expect("ambient credentials do not fail read-only validation; the code path must remain unauthenticated");
}

#[test]
fn authenticated_non_trading_is_explicit_opt_in() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(validate_authenticated_non_trading_smoke(&config, &l1_credentials()).is_err());
}

#[test]
fn sign_only_is_not_live_submit() {
    let config = OfficialSdkAdapterConfig {
        allow_sign_only_dry_run: true,
        allow_live_submit: true,
        ..OfficialSdkAdapterConfig::default()
    };
    assert!(validate_sign_only_dry_run(&config, &l1_credentials()).is_err());
}

#[test]
fn live_submit_preconditions_are_closed_by_default() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(validate_live_submit_preconditions(&config, true, true, true).is_err());
}

fn all_live_canary_preconditions() -> LiveCanaryPreconditions {
    LiveCanaryPreconditions {
        compile_feature_live_submit: true,
        env_allow_live_submit: true,
        config_allow_live_submit: true,
        kill_switch_open: true,
        runtime_worker_healthy: true,
        geoblock_allowed: true,
        repository_reservation_exists: true,
        idempotency_key_written: true,
        reconcile_worker_healthy: true,
        account_whitelisted: true,
        market_whitelisted: true,
        size_cap_ok: true,
        daily_cap_ok: true,
        operator_approved: true,
        cancel_only_fallback_ready: true,
    }
}

#[test]
fn live_submit_canary_requires_every_gate() {
    let mut preconditions = all_live_canary_preconditions();
    validate_live_submit_canary_preconditions(&preconditions).expect("all gates set");
    preconditions.operator_approved = false;
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("operator approval is mandatory");
    assert!(err.to_string().contains("operator approval is missing"));
}

#[test]
fn live_canary_default_preconditions_are_blocked_without_side_effects() {
    let preconditions = default_blocked_live_canary_preconditions();
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("default live canary preconditions must be blocked");
    assert!(
        err.to_string()
            .contains("live-submit compile feature disabled")
    );
    assert!(
        err.to_string()
            .contains("PMX_ALLOW_LIVE_SUBMIT is not enabled")
    );
    assert!(
        err.to_string()
            .contains("cancel-only fallback is not ready")
    );
}

#[test]
fn live_submit_canary_requires_cancel_only_fallback() {
    let mut preconditions = all_live_canary_preconditions();
    preconditions.cancel_only_fallback_ready = false;
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("cancel-only fallback is mandatory");
    assert!(
        err.to_string()
            .contains("cancel-only fallback is not ready")
    );
}

#[test]
fn live_canary_prep_freezes_on_remote_unknown_and_never_submits() {
    let decision = prepare_live_canary_decision(&LiveCanaryPrepInput {
        account_id: "acct-canary".into(),
        market_id: "market-canary".into(),
        order_size_units: 1,
        daily_used_units: 0,
        per_order_cap_units: 10,
        per_day_cap_units: 10,
        account_whitelist: vec!["acct-canary".into()],
        market_whitelist: vec!["market-canary".into()],
        operator_approval_id: Some("approval-1".into()),
        cancel_only_fallback_ready: true,
        remote_unknown_orders: 1,
    });
    assert!(decision.frozen);
    assert!(!decision.submit_allowed);
    assert!(!decision.live_side_effects);
    assert!(
        decision
            .reasons
            .contains(&"remote unknown freeze active".into())
    );
}

#[test]
fn live_canary_prep_requires_whitelist_caps_approval_and_cancel_fallback() {
    let decision = prepare_live_canary_decision(&LiveCanaryPrepInput {
        account_id: "acct-not-allowed".into(),
        market_id: "market-not-allowed".into(),
        order_size_units: 11,
        daily_used_units: 9,
        per_order_cap_units: 10,
        per_day_cap_units: 10,
        account_whitelist: vec!["acct-canary".into()],
        market_whitelist: vec!["market-canary".into()],
        operator_approval_id: None,
        cancel_only_fallback_ready: false,
        remote_unknown_orders: 0,
    });
    assert!(!decision.preconditions.account_whitelisted);
    assert!(!decision.preconditions.market_whitelisted);
    assert!(!decision.preconditions.size_cap_ok);
    assert!(!decision.preconditions.daily_cap_ok);
    assert!(!decision.preconditions.operator_approved);
    assert!(!decision.preconditions.cancel_only_fallback_ready);
    assert!(!decision.submit_allowed);
    assert!(!decision.live_side_effects);
}

#[test]
fn plan_mapping_normalizes_limit_orders() {
    let mapping =
        official_sdk_plan_to_builder_mapping(&sample_plan_limit()).expect("limit mapping");
    assert_eq!(mapping.side, "BUY");
    assert_eq!(mapping.order_kind, "LIMIT");
    assert_eq!(mapping.time_in_force.as_deref(), Some("GTC"));
    assert_eq!(mapping.limit_price.as_deref(), Some("0.55"));
}

#[test]
fn plan_mapping_preserves_metadata_without_exposing_signed_payload() {
    let mut plan = sample_plan_limit();
    plan.builder_attribution = Some("builder-code".into());
    plan.fee_rate_bps = Some("0".into());
    plan.funder = Some("deposit-wallet".into());
    plan.signer = Some("operator-signer".into());
    plan.signature_type = Some("EOA".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("metadata mapping");
    assert_eq!(mapping.builder_attribution.as_deref(), Some("builder-code"));
    assert_eq!(mapping.funder.as_deref(), Some("deposit-wallet"));
    assert_eq!(mapping.signature_type.as_deref(), Some("EOA"));
}

#[test]
fn plan_mapping_maps_ioc_to_sdk_fak() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("ioc".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("ioc mapping");
    assert_eq!(mapping.time_in_force.as_deref(), Some("FAK"));
}

#[test]
fn plan_mapping_supports_fok_limit_orders() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("fok".into());
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("fok mapping");
    assert_eq!(mapping.time_in_force.as_deref(), Some("FOK"));
}

#[test]
fn plan_mapping_rejects_gtd_until_expiration_path_exists() {
    let mut plan = sample_plan_limit();
    plan.time_in_force = Some("gtd".into());
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("gtd not wired");
    assert!(err.to_string().contains("GTD mapping requires"));
}

#[test]
fn plan_mapping_requires_market_amount() {
    let mut plan = sample_plan_limit();
    plan.order_kind = "MARKET".into();
    plan.limit_price = None;
    plan.size = None;
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("market must need amount");
    assert!(matches!(err, OfficialSdkAdapterError::InvalidInput(_)));
}

#[test]
fn plan_mapping_supports_market_amount() {
    let mut plan = sample_plan_limit();
    plan.order_kind = "market".into();
    plan.limit_price = None;
    plan.size = None;
    plan.amount = Some("12.5".into());
    plan.time_in_force = None;
    let mapping = official_sdk_plan_to_builder_mapping(&plan).expect("market mapping");
    assert_eq!(mapping.order_kind, "MARKET");
    assert_eq!(mapping.amount.as_deref(), Some("12.5"));
    assert!(mapping.time_in_force.is_none());
}

#[test]
fn plan_mapping_rejects_placeholder_token_id() {
    let mut plan = sample_plan_limit();
    plan.token_id = "replace-me".into();
    let err = official_sdk_plan_to_builder_mapping(&plan).expect_err("invalid token");
    assert!(matches!(err, OfficialSdkAdapterError::InvalidInput(_)));
}

#[test]
fn plan_mapping_rejects_invalid_limit_price_and_zero_size() {
    let mut over_one = sample_plan_limit();
    over_one.limit_price = Some("1.01".into());
    assert!(official_sdk_plan_to_builder_mapping(&over_one).is_err());

    let mut zero_size = sample_plan_limit();
    zero_size.size = Some("0".into());
    assert!(official_sdk_plan_to_builder_mapping(&zero_size).is_err());
}

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
fn sign_only_request_converts_to_limit_plan() {
    let request = SignOnlyDryRunRequest {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        token_id: "456".into(),
        side: "SELL".into(),
        size: "25".into(),
        limit_price: "0.61".into(),
    };
    let plan = request.into_plan_order();
    assert_eq!(plan.order_kind, "LIMIT");
    assert_eq!(plan.side, "SELL");
    assert_eq!(plan.time_in_force.as_deref(), Some("GTC"));
}

#[test]
fn sign_only_lifecycle_records_are_persistable_and_non_mutating() {
    let receipt = SignOnlyDryRunReceipt {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        signed_order_ref: "sign-only:exec-1:plan-hash-1:sig-abcd".into(),
        posted: false,
    };
    let records =
        sign_only_lifecycle_records_from_receipt(&receipt).expect("sign-only lifecycle records");
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|record| record.no_remote_side_effect));
    assert_eq!(
        records.last().unwrap().state,
        SignOnlyLifecycleState::SignedDryRun
    );
    assert_eq!(
        records.last().unwrap().signed_order_ref.as_deref(),
        Some("sign-only:exec-1:plan-hash-1:sig-abcd")
    );
}

#[test]
fn sign_only_lifecycle_rejects_posted_receipt() {
    let receipt = SignOnlyDryRunReceipt {
        account_id: AccountId("acct-1".into()),
        execution_id: ExecutionId("exec-1".into()),
        plan_hash: HashValue("plan-hash-1".into()),
        signed_order_ref: "sign-only:exec-1:plan-hash-1:sig-abcd".into(),
        posted: true,
    };
    assert!(sign_only_lifecycle_records_from_receipt(&receipt).is_err());
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
