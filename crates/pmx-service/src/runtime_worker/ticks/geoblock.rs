use pmx_runtime::{
    GeoblockEvaluationInput, RuntimeWorkerProviderSnapshot, evaluate_geoblock_status,
};
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

pub async fn record_geoblock_worker_tick<S>(
    store: &S,
    tick: GeoblockWorkerTick,
) -> Result<GeoblockWorkerTickReceipt, ServiceError>
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
            "geoblock worker ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_geoblock_status(GeoblockEvaluationInput {
        status: tick.status,
        observed_at: tick.observed_at,
        last_error: tick.last_error,
    });
    let provider_tick = record_runtime_worker_provider_snapshot(
        store,
        RuntimeWorkerProviderSnapshot {
            account_id: tick.account_id,
            lease_owner_id: tick.lease_owner_id,
            instance_id: tick.instance_id,
            market_websocket_connected: tick.market_websocket_connected,
            market_websocket_stale: tick.market_websocket_stale,
            user_websocket_connected: tick.user_websocket_connected,
            user_websocket_stale: tick.user_websocket_stale,
            geoblock_status: evaluation.status.clone(),
            resource_refresh_fresh: tick.resource_refresh_fresh,
            remote_unknown_orders: tick.remote_unknown_orders,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(GeoblockWorkerTickReceipt {
        evaluation,
        provider_tick,
    })
}
