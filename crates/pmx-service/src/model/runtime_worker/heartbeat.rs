use chrono::Utc;
use pmx_runtime::{HeartbeatLeaseCandidate, HeartbeatLeaseElection};
use serde::{Deserialize, Serialize};

use crate::RuntimeWorkerProviderTickReceipt;

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
pub struct HeartbeatLeaseStoreTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatLeaseStoreTickReceipt {
    pub election: HeartbeatLeaseElection,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
    pub candidates_loaded: usize,
    pub heartbeat_recorded: bool,
}
