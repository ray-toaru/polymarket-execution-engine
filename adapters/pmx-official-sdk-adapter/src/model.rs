use pmx_core::{AccountId, ExecutionId, GeoblockStatus, HashValue, SignOnlyLifecycleRecord};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const OFFICIAL_SDK_REPOSITORY: &str = "https://github.com/Polymarket/rs-clob-client-v2";
pub const OFFICIAL_SDK_CRATE: &str = "polymarket_client_sdk_v2";
pub const PINNED_OFFICIAL_SDK_VERSION: &str = "=0.6.0-canary.1";
pub const CLOB_V2_HOST: &str = "https://clob-v2.polymarket.com";
pub const ENV_RUN_AUTHENTICATED_SMOKE: &str = "PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE";
pub const ENV_RUN_SIGN_ONLY_DRY_RUN: &str = "PMX_RUN_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_SIGN_ONLY_DRY_RUN: &str = "PMX_ALLOW_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_LIVE_SUBMIT: &str = "PMX_ALLOW_LIVE_SUBMIT";
pub const ENV_ALLOW_LIVE_CANCEL: &str = "PMX_ALLOW_LIVE_CANCEL";
pub const ENV_SDK_CALL_TIMEOUT_SECS: &str = "PMX_SDK_CALL_TIMEOUT_SECS";
pub const REDACTED: &str = "[REDACTED]";
pub const CLOB_V2_COLLATERAL_SYMBOL: &str = "pUSD";
pub const CLOB_V2_SIGNING_PROTOCOL: &str = "CLOB_V2";

pub(crate) const PRIVATE_KEY_VAR_NAME: &str = "POLYMARKET_PRIVATE_KEY";
pub(crate) const L2_API_KEY_VAR: &str = "POLY_API_KEY";
pub(crate) const L2_API_SECRET_VAR: &str = "POLY_API_SECRET";
pub(crate) const L2_API_PASSPHRASE_VAR: &str = "POLY_API_PASSPHRASE";

pub(crate) fn env_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

pub(crate) fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyProfile {
    pub clob_host: String,
    pub collateral_symbol: String,
    pub signing_protocol: String,
    pub uses_deposit_wallet_order_path: bool,
    pub supports_builder_attribution: bool,
    pub supports_fee_metadata: bool,
    pub exposes_raw_signed_order: bool,
    pub may_post_order: bool,
    pub may_cancel_order: bool,
}

impl Default for OfficialSdkStandardSignOnlyProfile {
    fn default() -> Self {
        Self {
            clob_host: CLOB_V2_HOST.into(),
            collateral_symbol: CLOB_V2_COLLATERAL_SYMBOL.into(),
            signing_protocol: CLOB_V2_SIGNING_PROTOCOL.into(),
            uses_deposit_wallet_order_path: true,
            supports_builder_attribution: true,
            supports_fee_metadata: true,
            exposes_raw_signed_order: false,
            may_post_order: false,
            may_cancel_order: false,
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder_attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_rate_bps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_type: Option<String>,
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
    pub builder_attribution: Option<String>,
    pub fee_rate_bps: Option<String>,
    pub funder: Option<String>,
    pub signer: Option<String>,
    pub signature_type: Option<String>,
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
            builder_attribution: None,
            fee_rate_bps: None,
            funder: None,
            signer: None,
            signature_type: None,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyPlan {
    pub profile: OfficialSdkStandardSignOnlyProfile,
    pub mapping: OfficialSdkOrderBuilderMapping,
    pub signed_order_ref_namespace: String,
    pub exposes_raw_signed_order: bool,
    pub may_post_order: bool,
    pub may_cancel_order: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyConstruction {
    pub plan: OfficialSdkStandardSignOnlyPlan,
    pub plan_hash: HashValue,
    pub signed_order_ref: String,
    pub signed_order_digest: String,
    pub no_remote_side_effect: bool,
    pub raw_signed_order_exposed: bool,
    pub lifecycle_records: Vec<SignOnlyLifecycleRecord>,
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
#[serde(deny_unknown_fields)]
pub struct LiveCanaryPreconditions {
    pub compile_feature_live_submit: bool,
    pub env_allow_live_submit: bool,
    pub config_allow_live_submit: bool,
    pub kill_switch_open: bool,
    pub runtime_worker_healthy: bool,
    pub geoblock_allowed: bool,
    pub repository_reservation_exists: bool,
    pub idempotency_key_written: bool,
    pub reconcile_worker_healthy: bool,
    pub account_whitelisted: bool,
    pub market_whitelisted: bool,
    pub size_cap_ok: bool,
    pub daily_cap_ok: bool,
    pub operator_approved: bool,
    pub cancel_only_fallback_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCanaryPrepInput {
    pub account_id: String,
    pub market_id: String,
    pub order_size_units: u64,
    pub daily_used_units: u64,
    pub per_order_cap_units: u64,
    pub per_day_cap_units: u64,
    pub account_whitelist: Vec<String>,
    pub market_whitelist: Vec<String>,
    pub operator_approval_id: Option<String>,
    pub cancel_only_fallback_ready: bool,
    pub remote_unknown_orders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCanaryPrepDecision {
    pub preconditions: LiveCanaryPreconditions,
    pub frozen: bool,
    pub submit_allowed: bool,
    pub reasons: Vec<String>,
    pub live_side_effects: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialSdkReconcileDisposition {
    Healthy,
    ReconnectWebsocket,
    ReconcileRequired,
    Geoblocked,
}
