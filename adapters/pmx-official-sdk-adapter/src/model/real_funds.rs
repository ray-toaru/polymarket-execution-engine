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
    pub market_candidate_sha256: String,
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
pub struct ReviewedRealFundsCanaryReleaseDecision {
    pub schema_version: u64,
    pub decision_id: String,
    pub status: String,
    pub source_release: String,
    pub decision: String,
    pub decision_reason: String,
    pub scope: String,
    pub execution_style: String,
    pub expires_at: String,
    pub artifact_sha256: String,
    pub evidence_manifest_sha256: String,
    pub market_candidate_sha256: String,
    pub github_evidence: serde_json::Value,
    pub external_references: serde_json::Value,
    pub risk_limits: serde_json::Value,
    pub required_review_signals: serde_json::Value,
    pub live_submit_authorized: bool,
    pub live_cancel_authorized: bool,
    pub production_deployment_authorized: bool,
    pub real_funds_canary_authorized: bool,
    pub remote_side_effects_authorized: bool,
    pub allow_real_funds_canary: bool,
    pub reviewed_release_decision_present: bool,
    pub operator_identity_ref: String,
    pub secrets_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketCandidate {
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub order_type: String,
    pub active: bool,
    pub accepting_orders: bool,
    pub closed: bool,
    pub archived: bool,
    pub best_ask: String,
    pub ask_size: String,
    pub target_size: String,
    pub spread_bps: u64,
    pub min_order_size: String,
    pub exchange_rule_snapshot: ExchangeRuleSnapshot,
    pub liquidity_score: u64,
    pub book_snapshot_timestamp: String,
    pub human_review_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRuleSnapshot {
    pub schema_version: u64,
    pub venue: String,
    pub order_mode: String,
    pub order_type: String,
    pub side: String,
    pub target_size_semantics: String,
    pub min_share_size: String,
    pub marketable_buy_min_notional_usd: String,
    pub min_tick_size: String,
    pub source: String,
    pub captured_at: String,
    pub expires_at: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketRejectionCounts {
    pub inactive: u64,
    pub not_accepting_orders: u64,
    pub closed: u64,
    pub archived: u64,
    pub wrong_side: u64,
    pub wrong_order_type: u64,
    pub missing_book_snapshot_timestamp: u64,
    pub missing_human_review_ref: u64,
    pub missing_or_zero_target_size: u64,
    pub spread_too_wide: u64,
    pub missing_or_zero_best_ask: u64,
    pub insufficient_ask_size: u64,
    pub min_order_size_above_order_size: u64,
    pub exchange_rule_snapshot_invalid: u64,
    pub marketable_buy_notional_below_floor: u64,
    pub notional_over_cap: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketDiagnostics {
    pub market_validation_complete: bool,
    pub candidates_seen: u64,
    pub safe_candidates: u64,
    pub max_ask_size: Option<String>,
    pub min_spread_bps: Option<u64>,
    pub min_order_size_blocks: bool,
    pub rejection_counts: RealFundsCanaryMarketRejectionCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryMarketValidation {
    pub selection: Option<RealFundsCanaryMarketSelection>,
    pub diagnostics: RealFundsCanaryMarketDiagnostics,
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
    pub market_candidate_bound: bool,
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
    pub market_candidate_sha256: String,
    pub preconditions: RealFundsCanaryPreconditions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealFundsCanaryReceipt {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub approval_hash: String,
    pub market_candidate_sha256: String,
    pub idempotency_key: String,
    pub remote_order_id: Option<String>,
    pub remote_status: String,
    pub posted: bool,
    pub filled_or_matched: bool,
    pub cancelled: bool,
    pub remote_side_effects: bool,
    pub raw_signed_order_exposed: bool,
}
