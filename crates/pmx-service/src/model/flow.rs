use pmx_core::{
    ApprovalReceipt, ConstraintDecision, FeasibilitySnapshot, NormalizedIntent, SubmitReceipt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub normalized_intent: NormalizedIntent,
    pub snapshot: FeasibilitySnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionByIdRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanCommand {
    pub normalized_intent: NormalizedIntent,
    pub snapshot: FeasibilitySnapshot,
    pub decision: ConstraintDecision,
    pub approval: ApprovalReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanByIdCommand {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub approval: ApprovalReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlanCommand {
    pub execution_id: String,
    pub plan_hash: String,
    pub idempotency_key: String,
    pub mode: SubmitMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitMode {
    BlockedDryRun,
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    Accepted(SubmitReceipt),
    Replayed(SubmitReceipt),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCancelCommand {
    pub account_id: String,
    pub order_id: String,
    pub reason: String,
    pub correlation_id: Option<String>,
}
