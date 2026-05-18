use chrono::Utc;
use pmx_core::GeoblockStatus;
use pmx_runtime::GeoblockEvaluation;
use serde::{Deserialize, Serialize};

use crate::RuntimeWorkerProviderTickReceipt;

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
