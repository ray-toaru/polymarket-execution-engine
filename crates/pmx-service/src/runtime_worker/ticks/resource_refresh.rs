use pmx_runtime::{
    ResourceRefreshEvaluationInput, RuntimeWorkerProviderSnapshot,
    evaluate_resource_refresh_freshness,
};
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

pub async fn record_resource_refresh_worker_tick<S>(
    store: &S,
    tick: ResourceRefreshWorkerTick,
) -> Result<ResourceRefreshWorkerTickReceipt, ServiceError>
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
            "resource refresh worker ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observations: tick.observations,
        observed_at: tick.observed_at,
        stale_after_seconds: tick.stale_after_seconds,
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
            geoblock_status: tick.geoblock_status,
            resource_refresh_fresh: evaluation.fresh,
            remote_unknown_orders: tick.remote_unknown_orders,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(ResourceRefreshWorkerTickReceipt {
        evaluation,
        provider_tick,
    })
}
