use chrono::Utc;
use pmx_core::GeoblockStatus;
use pmx_runtime::{ResourceRefreshEvaluation, ResourceRefreshObservation};
use serde::{Deserialize, Serialize};

use crate::RuntimeWorkerProviderTickReceipt;

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
