use pmx_runtime::{
    RuntimeWorkerProviderSnapshot, WebSocketLivenessEvaluationInput, evaluate_websocket_liveness,
};
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

pub async fn record_websocket_liveness_worker_tick<S>(
    store: &S,
    tick: WebSocketLivenessWorkerTick,
) -> Result<WebSocketLivenessWorkerTickReceipt, ServiceError>
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
            "websocket liveness worker ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_websocket_liveness(WebSocketLivenessEvaluationInput {
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
            market_websocket_connected: evaluation.market_connected,
            market_websocket_stale: evaluation.market_stale,
            user_websocket_connected: evaluation.user_connected,
            user_websocket_stale: evaluation.user_stale,
            geoblock_status: tick.geoblock_status,
            resource_refresh_fresh: tick.resource_refresh_fresh,
            remote_unknown_orders: tick.remote_unknown_orders,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(WebSocketLivenessWorkerTickReceipt {
        evaluation,
        provider_tick,
    })
}
