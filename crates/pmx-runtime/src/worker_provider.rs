use chrono::{DateTime, Utc};
use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};

use crate::{RuntimeWorkerLoopInput, RuntimeWorkerLoopTick, runtime_worker_loop_tick};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerProviderSnapshot {
    pub account_id: String,
    pub lease_owner_id: String,
    pub instance_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub resource_refresh_fresh: bool,
    pub remote_unknown_orders: u32,
    pub observed_at: DateTime<Utc>,
    pub provider_name: String,
    pub no_trading_side_effect: bool,
}

impl RuntimeWorkerProviderSnapshot {
    pub fn into_loop_input(self) -> RuntimeWorkerLoopInput {
        RuntimeWorkerLoopInput {
            account_id: self.account_id,
            lease_owner_id: self.lease_owner_id,
            instance_id: self.instance_id,
            market_websocket_connected: self.market_websocket_connected,
            market_websocket_stale: self.market_websocket_stale,
            user_websocket_connected: self.user_websocket_connected,
            user_websocket_stale: self.user_websocket_stale,
            geoblock_status: self.geoblock_status,
            resource_refresh_fresh: self.resource_refresh_fresh,
            remote_unknown_orders: self.remote_unknown_orders,
            observed_at: self.observed_at,
        }
    }
}

pub trait RuntimeWorkerProvider {
    fn snapshot(&self) -> RuntimeWorkerProviderSnapshot;
}

pub fn runtime_worker_loop_tick_from_provider<P: RuntimeWorkerProvider>(
    provider: &P,
) -> RuntimeWorkerLoopTick {
    let snapshot = provider.snapshot();
    assert!(
        snapshot.no_trading_side_effect,
        "runtime worker providers must not trade"
    );
    runtime_worker_loop_tick(snapshot.into_loop_input())
}
