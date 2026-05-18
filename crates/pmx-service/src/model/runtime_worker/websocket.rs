use chrono::Utc;
use pmx_core::GeoblockStatus;
use pmx_runtime::{WebSocketLivenessEvaluation, WebSocketLivenessObservation};
use serde::{Deserialize, Serialize};

use crate::RuntimeWorkerProviderTickReceipt;

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
