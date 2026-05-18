use pmx_core::GeoblockStatus;
use pmx_runtime::{
    HeartbeatLeaseElectionInput, RuntimeWorkerProviderSnapshot, elect_heartbeat_lease_owner,
};
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

pub async fn record_heartbeat_lease_election_tick<S>(
    store: &S,
    tick: HeartbeatLeaseElectionTick,
) -> Result<HeartbeatLeaseElectionTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.account_id.trim().is_empty()
        || tick.provider_name.trim().is_empty()
        || tick.instance_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, provider_name and instance_id must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "heartbeat lease election ticks must not contain trading side effects".into(),
        ));
    }
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: tick.instance_id.clone(),
        candidates: tick.candidates,
        observed_at: tick.observed_at,
        stale_after_seconds: tick.stale_after_seconds,
    });
    let provider_tick = record_runtime_worker_provider_snapshot(
        store,
        RuntimeWorkerProviderSnapshot {
            account_id: tick.account_id,
            lease_owner_id: election.lease_owner_id.clone(),
            instance_id: tick.instance_id,
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(HeartbeatLeaseElectionTickReceipt {
        election,
        provider_tick,
    })
}
