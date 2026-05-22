use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::HashValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GeoblockStatus {
    Allowed,
    Blocked,
    Unknown,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    Healthy,
    Degraded,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollateralProfileStatus {
    Resolved,
    DefaultResolved,
    ExplicitMissing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStateSummary {
    pub geoblock_status: GeoblockStatus,
    pub worker_status: WorkerStatus,
    pub collateral_profile_status: CollateralProfileStatus,
    pub kill_switch_enabled: bool,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeasibilitySnapshot {
    pub snapshot_id: String,
    pub snapshot_hash: HashValue,
    pub normalized_intent_id: String,
    pub runtime_state: RuntimeStateSummary,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalScope {
    Shadow,
    ControlledCanary,
    LiveSubmit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceipt {
    pub approval_id: String,
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub approval_scope: ApprovalScope,
    pub approval_hash: HashValue,
    pub bound_artifact_sha256: HashValue,
    pub bound_evidence_manifest_sha256: HashValue,
    pub bound_snapshot_hash: HashValue,
    pub bound_decision_hash: HashValue,
    pub bound_plan_hash: Option<HashValue>,
    pub operator_identity_ref: String,
}
