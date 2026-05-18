use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkLivenessSnapshot {
    pub websocket_connected: bool,
    pub heartbeat_expected: bool,
    pub heartbeats_active: bool,
    pub geoblock_status: GeoblockStatus,
    pub remote_unknown_orders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialSdkReconcileDisposition {
    Healthy,
    ReconnectWebsocket,
    ReconcileRequired,
    Geoblocked,
}
