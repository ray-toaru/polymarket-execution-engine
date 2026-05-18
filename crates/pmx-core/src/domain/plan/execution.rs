use serde::{Deserialize, Serialize};

use crate::{AccountId, DecimalString, HashValue};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlanSummary {
    pub execution_id: String,
    pub account_id: AccountId,
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub plan_hash: HashValue,
    pub status: PlanStatus,
    pub max_exposure: DecimalString,
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
