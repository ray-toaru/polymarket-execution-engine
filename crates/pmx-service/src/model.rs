use chrono::Utc;
use pmx_core::*;
use pmx_runtime::{
    GeoblockEvaluation, HeartbeatLeaseCandidate, HeartbeatLeaseElection,
    ReconcileBacklogEvaluation, ResourceRefreshEvaluation, ResourceRefreshObservation,
    RuntimeSignal, RuntimeWorkerProviderSnapshot, WebSocketLivenessEvaluation,
    WebSocketLivenessObservation, WorkerCrashRecoveryEvaluation, WorkerCrashRecoveryObservation,
};
use pmx_store::StoreError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_CONTRACT_VERSION: &str = "1.0.0-draft";

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("in progress: retry_after_ms={retry_after_ms}")]
    InProgress { retry_after_ms: u64 },
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("internal error: {0}")]
    Internal(String),
}

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    Accepted(SubmitReceipt),
    Replayed(SubmitReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTick {
    pub worker_id: String,
    pub role: String,
    pub capability: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default)]
    pub signals: Vec<RuntimeSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTickReceipt {
    pub worker_id: String,
    pub capability: String,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerProviderTickReceipt {
    pub worker_id: String,
    pub provider_name: String,
    pub lease_owner_active: bool,
    pub submit_allowed_by_runtime: bool,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerContinuousTick {
    pub snapshots: Vec<RuntimeWorkerProviderSnapshot>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerContinuousTickReceipt {
    pub ticks_recorded: Vec<RuntimeWorkerProviderTickReceipt>,
    pub all_submit_allowed_by_runtime: bool,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatLeaseElectionTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub candidates: Vec<HeartbeatLeaseCandidate>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatLeaseElectionTickReceipt {
    pub election: HeartbeatLeaseElection,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRefreshWorkerTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub lease_owner_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub remote_unknown_orders: u32,
    pub observations: Vec<ResourceRefreshObservation>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRefreshWorkerTickReceipt {
    pub evaluation: ResourceRefreshEvaluation,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileBacklogWorkerTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub lease_owner_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_order_ids: Vec<String>,
    pub observed_at: chrono::DateTime<Utc>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileBacklogWorkerTickReceipt {
    pub evaluation: ReconcileBacklogEvaluation,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSocketLivenessWorkerTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub lease_owner_id: String,
    pub geoblock_status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_orders: u32,
    pub observations: Vec<WebSocketLivenessObservation>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSocketLivenessWorkerTickReceipt {
    pub evaluation: WebSocketLivenessEvaluation,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoblockWorkerTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub lease_owner_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_orders: u32,
    pub observed_at: chrono::DateTime<Utc>,
    pub last_error: Option<String>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoblockWorkerTickReceipt {
    pub evaluation: GeoblockEvaluation,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCrashRecoveryTick {
    pub account_id: String,
    pub worker_id: String,
    pub required_capabilities: Vec<String>,
    pub observations: Vec<WorkerCrashRecoveryObservation>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCrashRecoveryTickReceipt {
    pub evaluation: WorkerCrashRecoveryEvaluation,
    pub heartbeat_recorded: bool,
    pub observation_recorded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionRequest {
    pub execution_id: String,
    pub account_id: String,
    pub plan_hash: String,
    pub signed_order_ref: String,
    pub signed_order_digest: Option<String>,
    pub no_remote_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionReceipt {
    pub execution_id: String,
    pub signed_order_ref: String,
    pub signed_order_digest: Option<String>,
    pub lifecycle_records: Vec<SignOnlyLifecycleRecord>,
    pub no_remote_side_effect: bool,
}
