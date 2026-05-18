use serde::{Deserialize, Serialize};

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
