use serde::{Deserialize, Serialize};

use crate::{
    AccountId, ConditionId, DecimalString, HashValue, QuantityBound, Side, TimeInForce, TokenId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlanSummary {
    pub execution_id: String,
    pub account_id: AccountId,
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub snapshot_hash: HashValue,
    pub decision_id: String,
    pub decision_hash: HashValue,
    pub approval_id: String,
    pub approval_hash: HashValue,
    pub plan_hash: HashValue,
    pub status: PlanStatus,
    pub condition_id: ConditionId,
    pub token_id: TokenId,
    pub side: Side,
    pub quantity_bound: QuantityBound,
    pub limit_price: DecimalString,
    pub time_in_force: TimeInForce,
    pub collateral_profile_id: Option<String>,
    pub max_exposure: DecimalString,
    pub executor_version: String,
    pub contract_version: String,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitStatus {
    Accepted,
    Posted,
    PartialRemoteUnknown,
    RemoteUnknown,
    Rejected,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitReceipt {
    pub execution_id: String,
    pub receipt_id: String,
    pub status: SubmitStatus,
    pub executor_version: String,
    pub contract_version: String,
}
