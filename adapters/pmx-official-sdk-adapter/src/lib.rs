//! Official Polymarket SDK adapter boundary.
//!
//! This crate is the promotion target after the isolated SDK spike. It remains
//! outside the default execution-engine workspace so `pmx-core`, `pmx-policy`,
//! `pmx-store`, and the Python control plane cannot accidentally gain signing
//! or live trading capability.
//!
//! Safety posture:
//! - read-only SDK calls may be smoke-tested with no credentials;
//! - authenticated non-trading calls require explicit opt-in and real credentials;
//! - sign-only dry-runs require explicit opt-in and must never call `post_order`;
//! - live submit requires the explicit `live-submit` feature and runtime safety gates.

use pmx_core::{
    AccountId, ExecutionId, GeoblockStatus, HashValue, SignOnlyLifecycleEventKind,
    SignOnlyLifecycleRecord, SignOnlyLifecycleState, transition_sign_only_lifecycle,
    validate_limit_price_decimal_string, validate_positive_decimal_string,
};
use pmx_gateway::GatewayError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "authenticated-smoke")]
use polymarket_client_sdk_v2::clob::types::AssetType as SdkAssetType;
#[cfg(feature = "sign-only-dry-run")]
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};
#[cfg(feature = "sdk-typecheck")]
use polymarket_client_sdk_v2::error::{
    Error as SdkError, Geoblock as SdkGeoblock, Kind as SdkErrorKind, Status as SdkStatus,
};
#[cfg(feature = "sign-only-dry-run")]
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use anyhow::Context;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
#[cfg(feature = "authenticated-smoke")]
use polymarket_client_sdk_v2::clob::types::request::BalanceAllowanceRequest;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use std::str::FromStr;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use std::time::Duration;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use tokio::time;

pub const OFFICIAL_SDK_REPOSITORY: &str = "https://github.com/Polymarket/rs-clob-client-v2";
pub const OFFICIAL_SDK_CRATE: &str = "polymarket_client_sdk_v2";
pub const PINNED_OFFICIAL_SDK_VERSION: &str = "=0.6.0-canary.1";
pub const CLOB_V2_HOST: &str = "https://clob-v2.polymarket.com";
pub const ENV_RUN_AUTHENTICATED_SMOKE: &str = "PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE";
pub const ENV_RUN_SIGN_ONLY_DRY_RUN: &str = "PMX_RUN_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_SIGN_ONLY_DRY_RUN: &str = "PMX_ALLOW_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_LIVE_SUBMIT: &str = "PMX_ALLOW_LIVE_SUBMIT";
pub const ENV_SDK_CALL_TIMEOUT_SECS: &str = "PMX_SDK_CALL_TIMEOUT_SECS";
pub const REDACTED: &str = "[REDACTED]";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OfficialSdkAdapterError {
    #[error("operation disabled by adapter safety gate: {0}")]
    SafetyGate(String),
    #[error("required credential or environment value is missing: {0}")]
    MissingCredential(String),
    #[error("input is invalid for official SDK mapping: {0}")]
    InvalidInput(String),
    #[error("official SDK operation failed: {0}")]
    OperationFailed(String),
    #[error("SDK dependency is not enabled for this build")]
    SdkFeatureDisabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkAdapterConfig {
    pub clob_host: String,
    pub allow_read_only_smoke: bool,
    pub allow_authenticated_non_trading_smoke: bool,
    pub allow_sign_only_dry_run: bool,
    pub allow_live_submit: bool,
    pub require_kill_switch_open_for_live_submit: bool,
    pub require_repository_reservation_for_live_submit: bool,
    pub require_reconcile_worker_for_live_submit: bool,
}

impl Default for OfficialSdkAdapterConfig {
    fn default() -> Self {
        Self {
            clob_host: CLOB_V2_HOST.to_string(),
            allow_read_only_smoke: true,
            allow_authenticated_non_trading_smoke: false,
            allow_sign_only_dry_run: false,
            allow_live_submit: false,
            require_kill_switch_open_for_live_submit: true,
            require_repository_reservation_for_live_submit: true,
            require_reconcile_worker_for_live_submit: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCredentialSnapshot {
    pub has_l1_private_key: bool,
    pub has_l2_api_key: bool,
    pub has_l2_api_secret: bool,
    pub has_l2_passphrase: bool,
}

impl AdapterCredentialSnapshot {
    pub fn from_env() -> Self {
        Self {
            has_l1_private_key: env_present(PRIVATE_KEY_VAR_NAME),
            has_l2_api_key: env_present(L2_API_KEY_VAR),
            has_l2_api_secret: env_present(L2_API_SECRET_VAR),
            has_l2_passphrase: env_present(L2_API_PASSPHRASE_VAR),
        }
    }

    pub fn no_sensitive_material(&self) -> bool {
        !self.has_l1_private_key
            && !self.has_l2_api_key
            && !self.has_l2_api_secret
            && !self.has_l2_passphrase
    }

    pub fn has_authenticated_material(&self) -> bool {
        self.has_l1_private_key
            || (self.has_l2_api_key && self.has_l2_api_secret && self.has_l2_passphrase)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkPlanOrder {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub token_id: String,
    pub side: String,
    pub order_kind: String,
    pub limit_price: Option<String>,
    pub size: Option<String>,
    pub amount: Option<String>,
    pub time_in_force: Option<String>,
    pub post_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkOrderBuilderMapping {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub token_id: String,
    pub side: String,
    pub order_kind: String,
    pub limit_price: Option<String>,
    pub size: Option<String>,
    pub amount: Option<String>,
    pub time_in_force: Option<String>,
    pub post_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyDryRunRequest {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub token_id: String,
    pub side: String,
    pub size: String,
    pub limit_price: String,
}

impl SignOnlyDryRunRequest {
    pub fn into_plan_order(self) -> OfficialSdkPlanOrder {
        OfficialSdkPlanOrder {
            execution_id: self.execution_id,
            account_id: self.account_id,
            token_id: self.token_id,
            side: self.side,
            order_kind: "LIMIT".into(),
            limit_price: Some(self.limit_price),
            size: Some(self.size),
            amount: None,
            time_in_force: Some("GTC".into()),
            post_only: Some(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyDryRunReceipt {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub signed_order_ref: String,
    pub posted: bool,
}

/// Build a conservative sign-only lifecycle trace that can be persisted by the executor.
///
/// The trace deliberately terminates at `SignedDryRun`. It is invalid for this helper to
/// accept a receipt that claims it was posted, because sign-only dry-runs are non-mutating
/// probes and must not create remote Polymarket side effects.
pub fn sign_only_lifecycle_records_from_receipt(
    receipt: &SignOnlyDryRunReceipt,
) -> Result<Vec<SignOnlyLifecycleRecord>, OfficialSdkAdapterError> {
    if receipt.posted {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "sign-only receipt unexpectedly indicates remote posting".into(),
        ));
    }

    let s1 = transition_sign_only_lifecycle(
        SignOnlyLifecycleState::Planned,
        SignOnlyLifecycleEventKind::PrepareReservation,
    )
    .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;
    let s2 = transition_sign_only_lifecycle(s1.clone(), SignOnlyLifecycleEventKind::RequestSigning)
        .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;
    let s3 =
        transition_sign_only_lifecycle(s2.clone(), SignOnlyLifecycleEventKind::SignedWithoutPost)
            .map_err(|err| OfficialSdkAdapterError::InvalidInput(err.to_string()))?;

    Ok(vec![
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s1,
            event: SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s2,
            event: SignOnlyLifecycleEventKind::RequestSigning,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
        SignOnlyLifecycleRecord {
            execution_id: receipt.execution_id.clone(),
            account_id: receipt.account_id.clone(),
            state: s3,
            event: SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some(receipt.signed_order_ref.clone()),
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        },
    ])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedNonTradingSmokeReport {
    pub ok_status: String,
    pub server_time: i64,
    pub api_key_count: usize,
    pub closed_only: bool,
    pub balance_allowance_checked: bool,
    pub credential_snapshot: AdapterCredentialSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialSdkErrorCategory {
    RemoteRejected,
    RemoteUnknown,
    AuthenticationFailed,
    ValidationFailed,
    Geoblocked,
    WebSocketFailed,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkNormalizedError {
    pub category: OfficialSdkErrorCategory,
    pub retryable: bool,
    pub message: String,
    pub http_status: Option<u16>,
    pub geoblock_country: Option<String>,
    pub geoblock_region: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkLivenessSnapshot {
    pub websocket_connected: bool,
    pub heartbeat_expected: bool,
    pub heartbeats_active: bool,
    pub geoblock_status: GeoblockStatus,
    pub remote_unknown_orders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialSdkReconcileDisposition {
    Healthy,
    ReconnectWebsocket,
    ReconcileRequired,
    Geoblocked,
}

pub fn validate_read_only_smoke_environment(
    _credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    // Read-only smoke must construct an unauthenticated SDK client and must not consume ambient
    // credentials even when a developer shell has `.env` exported. Credential presence is therefore
    // not a failure by itself; tests must prove the read-only code path does not authenticate, sign,
    // post, cancel, or update remote state.
    Ok(())
}

pub fn validate_authenticated_non_trading_smoke(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if !config.allow_authenticated_non_trading_smoke || !env_flag(ENV_RUN_AUTHENTICATED_SMOKE) {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_AUTHENTICATED_SMOKE}=1 and config.allow_authenticated_non_trading_smoke=true"
        )));
    }
    if !credentials.has_authenticated_material() {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "authenticated non-trading smoke needs L1 or complete L2 credentials".into(),
        ));
    }
    Ok(())
}

pub fn validate_sign_only_dry_run(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if config.allow_live_submit || env_flag(ENV_ALLOW_LIVE_SUBMIT) || cfg!(feature = "live-submit")
    {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "sign-only dry-run must not run in a live-submit-enabled process".into(),
        ));
    }
    if !config.allow_sign_only_dry_run
        || !env_flag(ENV_RUN_SIGN_ONLY_DRY_RUN)
        || !env_flag(ENV_ALLOW_SIGN_ONLY_DRY_RUN)
    {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_SIGN_ONLY_DRY_RUN}=1, {ENV_ALLOW_SIGN_ONLY_DRY_RUN}=1 and config.allow_sign_only_dry_run=true"
        )));
    }
    if !credentials.has_l1_private_key {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "sign-only dry-run needs an L1 signer, but must not post the order".into(),
        ));
    }
    Ok(())
}

pub fn validate_live_submit_preconditions(
    config: &OfficialSdkAdapterConfig,
    kill_switch_open: bool,
    has_repository_reservation: bool,
    reconcile_worker_healthy: bool,
) -> Result<(), OfficialSdkAdapterError> {
    if !cfg!(feature = "live-submit")
        || !env_flag(ENV_ALLOW_LIVE_SUBMIT)
        || !config.allow_live_submit
    {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "live submit requires live-submit feature, PMX_ALLOW_LIVE_SUBMIT=1 and config.allow_live_submit=true".into(),
        ));
    }
    if config.require_kill_switch_open_for_live_submit && !kill_switch_open {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "kill switch is not explicitly open".into(),
        ));
    }
    if config.require_repository_reservation_for_live_submit && !has_repository_reservation {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "repository reservation is missing".into(),
        ));
    }
    if config.require_reconcile_worker_for_live_submit && !reconcile_worker_healthy {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "reconcile worker is not healthy".into(),
        ));
    }
    Ok(())
}

pub fn official_sdk_plan_to_builder_mapping(
    plan: &OfficialSdkPlanOrder,
) -> Result<OfficialSdkOrderBuilderMapping, OfficialSdkAdapterError> {
    let normalized_side = normalize_side(&plan.side)?;
    let normalized_kind = normalize_order_kind(&plan.order_kind)?;
    let normalized_tif = normalize_time_in_force(plan.time_in_force.as_deref(), &normalized_kind)?;
    validate_token_id(&plan.token_id)?;

    match normalized_kind.as_str() {
        "LIMIT" => {
            let limit_price = require_non_empty(plan.limit_price.as_deref(), "limit_price")?;
            let size = require_non_empty(plan.size.as_deref(), "size")?;
            validate_limit_price_for_sdk(limit_price)?;
            validate_positive_quantity_for_sdk(size, "size")?;
        }
        "MARKET" => {
            let amount = require_non_empty(plan.amount.as_deref(), "amount")?;
            validate_positive_quantity_for_sdk(amount, "amount")?;
        }
        _ => unreachable!("normalize_order_kind restricts allowed values"),
    }

    Ok(OfficialSdkOrderBuilderMapping {
        execution_id: plan.execution_id.clone(),
        account_id: plan.account_id.clone(),
        token_id: plan.token_id.clone(),
        side: normalized_side,
        order_kind: normalized_kind,
        limit_price: clone_non_empty(plan.limit_price.as_deref()),
        size: clone_non_empty(plan.size.as_deref()),
        amount: clone_non_empty(plan.amount.as_deref()),
        time_in_force: normalized_tif,
        post_only: plan.post_only.unwrap_or(false),
    })
}

pub fn assess_sdk_liveness(
    snapshot: &OfficialSdkLivenessSnapshot,
) -> OfficialSdkReconcileDisposition {
    if snapshot.geoblock_status == GeoblockStatus::Blocked {
        return OfficialSdkReconcileDisposition::Geoblocked;
    }
    if !snapshot.websocket_connected || (snapshot.heartbeat_expected && !snapshot.heartbeats_active)
    {
        return OfficialSdkReconcileDisposition::ReconnectWebsocket;
    }
    if snapshot.remote_unknown_orders > 0 {
        return OfficialSdkReconcileDisposition::ReconcileRequired;
    }
    OfficialSdkReconcileDisposition::Healthy
}

pub fn gateway_error_from_normalized_sdk_error(
    normalized: &OfficialSdkNormalizedError,
) -> GatewayError {
    match normalized.category {
        OfficialSdkErrorCategory::AuthenticationFailed => GatewayError::AuthenticationFailed,
        OfficialSdkErrorCategory::ValidationFailed | OfficialSdkErrorCategory::RemoteRejected => {
            GatewayError::RemoteRejected(redact_sensitive_text(&normalized.message))
        }
        OfficialSdkErrorCategory::RemoteUnknown
        | OfficialSdkErrorCategory::WebSocketFailed
        | OfficialSdkErrorCategory::Geoblocked
        | OfficialSdkErrorCategory::Internal => {
            GatewayError::RemoteUnknown(redact_sensitive_text(&normalized.message))
        }
    }
}

fn redact_assignment_value(input: &str, key: &str) -> String {
    let marker = format!("{key}=");
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(&marker) {
        out.push_str(&rest[..idx]);
        out.push_str(&marker);
        out.push_str(REDACTED);
        let after = &rest[idx + marker.len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '&'))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn redact_known_env_values(input: &str) -> String {
    let mut out = input.to_owned();
    for key in [
        PRIVATE_KEY_VAR_NAME,
        L2_API_KEY_VAR,
        L2_API_SECRET_VAR,
        L2_API_PASSPHRASE_VAR,
    ] {
        if let Ok(value) = std::env::var(key)
            && value.len() >= 4
        {
            out = out.replace(&value, REDACTED);
        }
        out = redact_assignment_value(&out, key);
    }
    out
}

fn looks_like_hex_private_key(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '(' | '"' | '\''));
    let Some(hex) = trimmed.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn redact_sensitive_text(input: &str) -> String {
    let env_redacted = redact_known_env_values(input);
    env_redacted
        .split_whitespace()
        .map(|token| {
            if looks_like_hex_private_key(token) {
                "0x[REDACTED]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn redact_normalized_error(error: &OfficialSdkNormalizedError) -> OfficialSdkNormalizedError {
    let mut redacted = error.clone();
    redacted.message = redact_sensitive_text(&redacted.message);
    redacted
}

#[cfg(feature = "sdk-typecheck")]
pub fn sdk_type_markers() -> Vec<&'static str> {
    vec![
        std::any::type_name::<polymarket_client_sdk_v2::clob::Client>(),
        std::any::type_name::<polymarket_client_sdk_v2::clob::Config>(),
        std::any::type_name::<polymarket_client_sdk_v2::types::Decimal>(),
    ]
}

#[cfg(not(feature = "sdk-typecheck"))]
pub fn sdk_type_markers() -> Result<(), OfficialSdkAdapterError> {
    Err(OfficialSdkAdapterError::SdkFeatureDisabled)
}

#[cfg(feature = "sdk-typecheck")]
pub fn normalize_sdk_error(error: &SdkError) -> OfficialSdkNormalizedError {
    match error.kind() {
        SdkErrorKind::Validation => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::ValidationFailed,
            retryable: false,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Synchronization => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Geoblock => {
            let geoblock = error.downcast_ref::<SdkGeoblock>();
            OfficialSdkNormalizedError {
                category: OfficialSdkErrorCategory::Geoblocked,
                retryable: false,
                message: error.to_string(),
                http_status: None,
                geoblock_country: geoblock.map(|g| g.country.clone()),
                geoblock_region: geoblock.map(|g| g.region.clone()),
            }
        }
        SdkErrorKind::WebSocket => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::WebSocketFailed,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Status => {
            let status = error.downcast_ref::<SdkStatus>();
            let code = status.map(|s| s.status_code.as_u16());
            let category = match code {
                Some(401 | 403) => OfficialSdkErrorCategory::AuthenticationFailed,
                Some(408 | 429 | 500..=599) => OfficialSdkErrorCategory::RemoteUnknown,
                _ => OfficialSdkErrorCategory::RemoteRejected,
            };
            let retryable = matches!(code, Some(408 | 429 | 500..=599));
            OfficialSdkNormalizedError {
                category,
                retryable,
                message: error.to_string(),
                http_status: code,
                geoblock_country: None,
                geoblock_region: None,
            }
        }
        SdkErrorKind::Internal => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        _ => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
    }
}

#[cfg(feature = "sdk-typecheck")]
pub fn geoblock_status_from_sdk(blocked: bool) -> GeoblockStatus {
    if blocked {
        GeoblockStatus::Blocked
    } else {
        GeoblockStatus::Allowed
    }
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
fn sdk_call_timeout() -> Duration {
    let parsed = std::env::var(ENV_SDK_CALL_TIMEOUT_SECS)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|secs| *secs > 0);
    Duration::from_secs(parsed.unwrap_or(10))
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
fn signer_from_env() -> anyhow::Result<impl polymarket_client_sdk_v2::auth::Signer + Clone> {
    let private_key = std::env::var(PRIVATE_KEY_VAR)
        .with_context(|| format!("missing {PRIVATE_KEY_VAR} for official SDK signer"))?;
    let signer = LocalSigner::from_str(&private_key)
        .context("invalid POLYMARKET_PRIVATE_KEY for official SDK signer")?
        .with_chain_id(Some(POLYGON));
    Ok(signer)
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
fn sdk_credentials_from_env() -> anyhow::Result<Option<SdkCredentials>> {
    match (
        std::env::var(L2_API_KEY_VAR).ok(),
        std::env::var(L2_API_SECRET_VAR).ok(),
        std::env::var(L2_API_PASSPHRASE_VAR).ok(),
    ) {
        (Some(key), Some(secret), Some(passphrase)) => {
            let uuid = Uuid::parse_str(&key).context("invalid POLY_API_KEY UUID")?;
            Ok(Some(SdkCredentials::new(uuid, secret, passphrase)))
        }
        (None, None, None) => Ok(None),
        _ => Err(anyhow::anyhow!(
            "partial L2 credential material present; require POLY_API_KEY, POLY_API_SECRET and POLY_API_PASSPHRASE together"
        )),
    }
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
async fn authenticated_sdk_client(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<
    SdkClient<
        polymarket_client_sdk_v2::auth::state::Authenticated<
            polymarket_client_sdk_v2::auth::Normal,
        >,
    >,
> {
    let signer = signer_from_env()?;
    let mut builder = SdkClient::new(
        &config.clob_host,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating official SDK client")?
    .authentication_builder(&signer);
    if let Some(credentials) = sdk_credentials_from_env()? {
        builder = builder.credentials(credentials);
    }
    let timeout = sdk_call_timeout();
    let client = time::timeout(timeout, builder.authenticate())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK authentication timed out after {timeout:?}"))?
        .context("official SDK authentication failed")?;
    Ok(client)
}

#[cfg(all(feature = "sign-only-dry-run", test))]
async fn discover_active_token_id(config: &OfficialSdkAdapterConfig) -> anyhow::Result<String> {
    let client = SdkClient::new(
        &config.clob_host,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating public official SDK client")?;
    let timeout = sdk_call_timeout();
    let markets = time::timeout(timeout, client.simplified_markets(None))
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK simplified_markets() timed out after {timeout:?}")
        })?
        .context("official SDK simplified_markets() failed")?;

    let token_id = markets
        .data
        .iter()
        .find(|market| {
            market.active && !market.closed && !market.archived && market.accepting_orders
        })
        .and_then(|market| market.tokens.first())
        .map(|token| token.token_id.to_string())
        .or_else(|| {
            markets.data.iter().find_map(|market| {
                market
                    .tokens
                    .first()
                    .map(|token| token.token_id.to_string())
            })
        })
        .ok_or_else(|| {
            anyhow::anyhow!("no simplified market token_id available for sign-only dry-run")
        })?;

    Ok(token_id)
}

#[cfg(feature = "authenticated-smoke")]
pub async fn run_authenticated_non_trading_sdk_smoke(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<AuthenticatedNonTradingSmokeReport> {
    let credentials = AdapterCredentialSnapshot::from_env();
    validate_authenticated_non_trading_smoke(config, &credentials)?;
    if !credentials.has_l1_private_key {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "authenticated non-trading smoke currently requires POLYMARKET_PRIVATE_KEY for SDK authentication".into(),
        )
        .into());
    }

    let client = authenticated_sdk_client(config).await?;
    let timeout = sdk_call_timeout();

    let ok_status = time::timeout(timeout, client.ok())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK ok() timed out after {timeout:?}"))?
        .context("official SDK ok() failed")?;
    let server_time = time::timeout(timeout, client.server_time())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK server_time() timed out after {timeout:?}"))?
        .context("official SDK server_time() failed")?;
    let readonly_api_keys = time::timeout(timeout, client.readonly_api_keys())
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK readonly_api_keys() timed out after {timeout:?}")
        })?
        .context("official SDK readonly_api_keys() failed")?;
    let closed_only = time::timeout(timeout, client.closed_only_mode())
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK closed_only_mode() timed out after {timeout:?}")
        })?
        .context("official SDK closed_only_mode() failed")?;
    let _balance_allowance = time::timeout(
        timeout,
        client.balance_allowance(
            BalanceAllowanceRequest::builder()
                .asset_type(SdkAssetType::Collateral)
                .build(),
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK balance_allowance() timed out after {timeout:?}"))?
    .context("official SDK balance_allowance() failed")?;

    Ok(AuthenticatedNonTradingSmokeReport {
        ok_status,
        server_time,
        api_key_count: readonly_api_keys.len(),
        closed_only: closed_only.closed_only,
        balance_allowance_checked: true,
        credential_snapshot: credentials,
    })
}

#[cfg(feature = "sign-only-dry-run")]
pub async fn run_sign_only_dry_run(
    config: &OfficialSdkAdapterConfig,
    request: SignOnlyDryRunRequest,
) -> anyhow::Result<SignOnlyDryRunReceipt> {
    let credentials = AdapterCredentialSnapshot::from_env();
    validate_sign_only_dry_run(config, &credentials)?;

    let mapping = official_sdk_plan_to_builder_mapping(&request.clone().into_plan_order())?;
    let signer = signer_from_env()?;
    let client = authenticated_sdk_client(config).await?;
    let timeout = sdk_call_timeout();

    let token_id = SdkU256::from_str(&mapping.token_id)
        .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid token_id: {e}")))?;
    let price = SdkDecimal::from_str(
        mapping
            .limit_price
            .as_deref()
            .ok_or_else(|| OfficialSdkAdapterError::InvalidInput("missing limit_price".into()))?,
    )
    .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid limit_price: {e}")))?;
    let size = SdkDecimal::from_str(
        mapping
            .size
            .as_deref()
            .ok_or_else(|| OfficialSdkAdapterError::InvalidInput("missing size".into()))?,
    )
    .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid size: {e}")))?;
    let side = parse_sdk_side(&mapping.side)?;
    let order_type =
        parse_sdk_order_type(mapping.time_in_force.as_deref().ok_or_else(|| {
            OfficialSdkAdapterError::InvalidInput("missing time_in_force".into())
        })?)?;

    let signable = time::timeout(
        timeout,
        client
            .limit_order()
            .token_id(token_id)
            .price(price)
            .size(size)
            .side(side)
            .order_type(order_type)
            .post_only(mapping.post_only)
            .build(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK limit_order().build() timed out after {timeout:?}"))?
    .context("official SDK limit order build failed")?;

    let signed = time::timeout(timeout, client.sign(&signer, signable))
        .await
        .map_err(|_| anyhow::anyhow!("official SDK sign() timed out after {timeout:?}"))?
        .context("official SDK sign() failed")?;

    let signed_order_ref = format!(
        "sign-only:{}:{}:{}",
        request.execution_id.0,
        request.plan_hash.0,
        signature_fingerprint(&signed.signature.to_string())
    );

    Ok(SignOnlyDryRunReceipt {
        account_id: request.account_id,
        execution_id: request.execution_id,
        plan_hash: request.plan_hash,
        signed_order_ref,
        posted: false,
    })
}

fn env_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

fn require_non_empty<'a>(
    value: Option<&'a str>,
    field: &str,
) -> Result<&'a str, OfficialSdkAdapterError> {
    let raw = value
        .ok_or_else(|| OfficialSdkAdapterError::InvalidInput(format!("{field} is required")))?;
    if raw.trim().is_empty() || raw != raw.trim() {
        return Err(OfficialSdkAdapterError::InvalidInput(format!(
            "{field} is required"
        )));
    }
    Ok(raw)
}

fn validate_token_id(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed != raw || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(OfficialSdkAdapterError::InvalidInput(format!(
            "invalid token_id for official SDK order builder: {raw}"
        )));
    }
    Ok(())
}

fn validate_limit_price_for_sdk(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    validate_limit_price_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid limit_price for official SDK order builder: {raw}"
        ))
    })
}

fn validate_positive_quantity_for_sdk(
    raw: &str,
    field: &str,
) -> Result<(), OfficialSdkAdapterError> {
    validate_positive_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid {field} for official SDK order builder: {raw}"
        ))
    })
}

fn clone_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_order_kind(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "LIMIT" | "MARKET" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported order_kind: {raw}"
        ))),
    }
}

fn normalize_side(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "BUY" | "SELL" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

fn normalize_time_in_force(
    raw: Option<&str>,
    order_kind: &str,
) -> Result<Option<String>, OfficialSdkAdapterError> {
    if order_kind == "MARKET" {
        return Ok(None);
    }
    let normalized = raw.unwrap_or("GTC").trim().to_ascii_uppercase();
    match normalized.as_str() {
        "GTC" | "FOK" | "FAK" => Ok(Some(normalized)),
        "GTD" => Err(OfficialSdkAdapterError::InvalidInput(
            "GTD mapping requires an explicit expiration path that is not wired in v0.20".into(),
        )),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {normalized}"
        ))),
    }
}

#[cfg(feature = "sign-only-dry-run")]
fn parse_sdk_side(raw: &str) -> Result<SdkSide, OfficialSdkAdapterError> {
    match raw {
        "BUY" => Ok(SdkSide::Buy),
        "SELL" => Ok(SdkSide::Sell),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

#[cfg(feature = "sign-only-dry-run")]
fn parse_sdk_order_type(raw: &str) -> Result<SdkOrderType, OfficialSdkAdapterError> {
    match raw {
        "GTC" => Ok(SdkOrderType::GTC),
        "FOK" => Ok(SdkOrderType::FOK),
        "FAK" => Ok(SdkOrderType::FAK),
        "GTD" => Err(OfficialSdkAdapterError::InvalidInput(
            "GTD sign-only is not wired in v0.20".into(),
        )),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {raw}"
        ))),
    }
}

#[cfg(feature = "sign-only-dry-run")]
fn signature_fingerprint(signature: &str) -> String {
    let trimmed = signature.strip_prefix("0x").unwrap_or(signature);
    let head = trimmed.get(..16).unwrap_or(trimmed);
    format!("sig-{head}")
}

const PRIVATE_KEY_VAR_NAME: &str = "POLYMARKET_PRIVATE_KEY";
const L2_API_KEY_VAR: &str = "POLY_API_KEY";
const L2_API_SECRET_VAR: &str = "POLY_API_SECRET";
const L2_API_PASSPHRASE_VAR: &str = "POLY_API_PASSPHRASE";

#[cfg(test)]
mod tests {
    use super::*;

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
    fn read_only_smoke_ignores_ambient_credentials_but_must_remain_unauthenticated() {
        validate_read_only_smoke_environment(&empty_credentials())
            .expect("empty credentials allowed");
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
        let records = sign_only_lifecycle_records_from_receipt(&receipt)
            .expect("sign-only lifecycle records");
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
            .expect(
                "authenticated non-trading smoke should succeed when env is explicitly enabled",
            );
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
}
