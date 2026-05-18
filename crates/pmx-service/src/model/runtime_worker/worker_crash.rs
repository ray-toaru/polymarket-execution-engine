use chrono::Utc;
use pmx_runtime::{WorkerCrashRecoveryEvaluation, WorkerCrashRecoveryObservation};
use serde::{Deserialize, Serialize};

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
