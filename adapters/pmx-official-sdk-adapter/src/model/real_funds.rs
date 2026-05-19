use pmx_core::{AccountId, ExecutionId, HashValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryApproval {
    pub approval_id: String,
    pub approval_hash: String,
    pub account_id: AccountId,
    pub scope: String,
    pub expires_at: String,
    pub artifact_sha256: String,
    pub evidence_manifest_sha256: String,
    pub max_order_notional_usd: String,
    pub max_daily_notional_usd: String,
    pub execution_style: String,
    pub operator_identity_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryRiskLimits {
    pub max_order_notional_usd: String,
    pub max_daily_notional_usd: String,
    pub daily_used_notional_usd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketCandidate {
    pub market_id: String,
    pub token_id: String,
    pub active: bool,
    pub accepting_orders: bool,
    pub closed: bool,
    pub archived: bool,
    pub best_ask: String,
    pub ask_size: String,
    pub spread_bps: u64,
    pub min_order_size: String,
    pub liquidity_score: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketSelection {
    pub market_id: String,
    pub token_id: String,
    pub limit_price: String,
    pub size: String,
    pub notional_usd: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryPreconditions {
    pub live_canary: super::LiveCanaryPreconditions,
    pub env_allow_real_funds_canary: bool,
    pub config_allow_real_funds_canary: bool,
    pub approval_valid: bool,
    pub approval_scope_matches: bool,
    pub approval_not_expired: bool,
    pub artifact_bound: bool,
    pub evidence_manifest_bound: bool,
    pub max_order_notional_ok: bool,
    pub max_daily_notional_ok: bool,
    pub execution_style_fok_limit_fill: bool,
    pub balance_allowance_checked: bool,
    pub selected_market_safe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryRequest {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub idempotency_key: String,
    pub approval: RealFundsCanaryApproval,
    pub risk_limits: RealFundsCanaryRiskLimits,
    pub market: RealFundsCanaryMarketSelection,
    pub preconditions: RealFundsCanaryPreconditions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryReceipt {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub approval_hash: String,
    pub idempotency_key: String,
    pub remote_order_id: Option<String>,
    pub remote_status: String,
    pub posted: bool,
    pub filled_or_matched: bool,
    pub cancelled: bool,
    pub remote_side_effects: bool,
    pub raw_signed_order_exposed: bool,
}
