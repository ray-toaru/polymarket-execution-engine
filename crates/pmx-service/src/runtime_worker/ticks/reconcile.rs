use pmx_runtime::{
    ReconcileBacklogEvaluationInput, RuntimeWorkerProviderSnapshot, evaluate_reconcile_backlog,
};
use pmx_store::{
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, RuntimeWorkerHealthStore,
    RuntimeWorkerObservationStore,
};

use crate::model::*;
use crate::runtime_worker::record_runtime_worker_provider_snapshot;

pub async fn record_reconcile_backlog_worker_tick<S>(
    store: &S,
    tick: ReconcileBacklogWorkerTick,
) -> Result<ReconcileBacklogWorkerTickReceipt, ServiceError>
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
            "reconcile backlog worker ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_reconcile_backlog(ReconcileBacklogEvaluationInput {
        remote_unknown_order_ids: tick.remote_unknown_order_ids,
        observed_at: tick.observed_at,
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
            resource_refresh_fresh: tick.resource_refresh_fresh,
            remote_unknown_orders: evaluation.remote_unknown_orders,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(ReconcileBacklogWorkerTickReceipt {
        evaluation,
        provider_tick,
    })
}

pub async fn record_reconcile_backlog_from_order_lifecycle<S>(
    store: &S,
    mut tick: ReconcileBacklogWorkerTick,
) -> Result<ReconcileBacklogWorkerTickReceipt, ServiceError>
where
    S: OrderReconcileBacklogStore
        + RuntimeWorkerHealthStore
        + RuntimeWorkerObservationStore
        + Send
        + Sync,
{
    if tick.account_id.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "account_id must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "reconcile backlog lifecycle reader must not contain trading side effects".into(),
        ));
    }
    let backlog = store
        .list_reconcile_backlog_orders(&OrderReconcileBacklogQuery {
            account_id: tick.account_id.clone(),
            limit: 500,
        })
        .await?;
    tick.remote_unknown_order_ids = backlog.into_iter().map(|order| order.order_id).collect();
    record_reconcile_backlog_worker_tick(store, tick).await
}
