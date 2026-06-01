use serde::{Deserialize, Serialize};

use crate::HashValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionStatus {
    Allow,
    Block,
    CloseOnly,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockReason {
    KillSwitchOn,
    GeoblockBlocked,
    GeoblockUnknown,
    GeoblockError,
    WorkerDegraded,
    WorkerStale,
    WorkerUnknown,
    CollateralProfileMissing,
    CollateralProfileUnknown,
    UnsupportedQuantityBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintDecision {
    pub decision_id: String,
    pub decision_hash: HashValue,
    pub correlation_id: Option<String>,
    pub status: DecisionStatus,
    pub reasons: Vec<BlockReason>,
}
