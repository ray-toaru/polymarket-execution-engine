use chrono::{DateTime, Utc};
use pmx_core::GeoblockStatus;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, interval};

mod evaluation;
mod health;

pub use evaluation::*;
pub use health::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerLoopInput {
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
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeWorkerLoopTick {
    pub account_id: String,
    pub lease_owner_active: bool,
    pub signals: Vec<RuntimeSignal>,
    pub actions: Vec<RuntimeWorkerAction>,
    pub submit_allowed_by_runtime: bool,
}

/// Build one deterministic runtime worker tick from observed worker inputs.
///
/// Network workers and store crates own I/O. This function is the pure boundary
/// that makes disconnects, stale leases, geoblocks, stale resource refreshes,
/// and reconcile backlog consistently fail closed before submit decisions.
pub fn runtime_worker_loop_tick(input: RuntimeWorkerLoopInput) -> RuntimeWorkerLoopTick {
    let lease_owner_active = input.lease_owner_id == input.instance_id;
    let observed_at = Some(input.observed_at);
    let geoblock_allowed = matches!(input.geoblock_status, GeoblockStatus::Allowed);
    let signals = vec![
        RuntimeSignal::HeartbeatLease {
            active: lease_owner_active,
            last_observed_at: observed_at,
            last_error: (!lease_owner_active).then(|| "stale lease owner".into()),
        },
        RuntimeSignal::WebSocket {
            channel: WebSocketChannel::Market,
            connected: input.market_websocket_connected,
            stale: input.market_websocket_stale,
            last_observed_at: observed_at,
            last_error: (!input.market_websocket_connected || input.market_websocket_stale)
                .then(|| "market websocket unhealthy".into()),
        },
        RuntimeSignal::WebSocket {
            channel: WebSocketChannel::User,
            connected: input.user_websocket_connected,
            stale: input.user_websocket_stale,
            last_observed_at: observed_at,
            last_error: (!input.user_websocket_connected || input.user_websocket_stale)
                .then(|| "user websocket unhealthy".into()),
        },
        RuntimeSignal::Geoblock {
            status: input.geoblock_status,
            last_observed_at: observed_at,
            last_error: (!geoblock_allowed).then(|| "geoblock not allowed".into()),
        },
        RuntimeSignal::ResourceRefresh {
            fresh: input.resource_refresh_fresh,
            last_observed_at: observed_at,
            last_error: (!input.resource_refresh_fresh).then(|| "resource refresh stale".into()),
        },
        RuntimeSignal::ReconcileBacklog {
            remote_unknown_orders: input.remote_unknown_orders,
            last_observed_at: observed_at,
            last_error: (input.remote_unknown_orders > 0).then(|| "remote unknown backlog".into()),
        },
    ];
    let actions = worker_actions_from_runtime_signals(&signals);
    let submit_allowed_by_runtime = actions.iter().all(|action| !action.should_fail_closed);
    RuntimeWorkerLoopTick {
        account_id: input.account_id,
        lease_owner_active,
        signals,
        actions,
        submit_allowed_by_runtime,
    }
}

pub async fn run_placeholder_worker(worker_id: String) {
    let mut ticker = interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;
        let _heartbeat = WorkerHeartbeat {
            worker_id: worker_id.clone(),
            role: WorkerRole::Heartbeat,
            capability: "heartbeat".to_string(),
            observed_at: Utc::now(),
            last_error: None,
        };
        // v0.1 placeholder. Real implementation persists heartbeat to worker_health.
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;
