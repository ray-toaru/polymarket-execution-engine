use pmx_core::*;
use pmx_runtime::{
    GeoblockEvaluationInput, HeartbeatLeaseElectionInput, ReconcileBacklogEvaluationInput,
    ResourceRefreshEvaluationInput, RuntimeWorkerProviderSnapshot,
    WebSocketLivenessEvaluationInput, WorkerCrashRecoveryEvaluationInput,
    elect_heartbeat_lease_owner, evaluate_geoblock_status, evaluate_reconcile_backlog,
    evaluate_resource_refresh_freshness, evaluate_websocket_liveness,
    evaluate_worker_crash_recovery,
};
use pmx_store::{
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, RuntimeWorkerHealthStore,
    RuntimeWorkerHeartbeat, RuntimeWorkerObservation, RuntimeWorkerObservationStore,
};

use super::record_runtime_worker_provider_snapshot;
use crate::model::*;

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

pub async fn record_worker_crash_recovery_tick<S>(
    store: &S,
    tick: WorkerCrashRecoveryTick,
) -> Result<WorkerCrashRecoveryTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.account_id.trim().is_empty() || tick.worker_id.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "account_id and worker_id must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "worker crash recovery ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_worker_crash_recovery(WorkerCrashRecoveryEvaluationInput {
        observations: tick.observations,
        required_capabilities: tick.required_capabilities,
        observed_at: tick.observed_at,
        stale_after_seconds: tick.stale_after_seconds,
    });
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: tick.worker_id,
            role: "WorkerCrashRecovery".into(),
            capability: "worker-crash-recovery".into(),
            status: if evaluation.recovered {
                "HEALTHY".into()
            } else {
                "STALE".into()
            },
            last_heartbeat_at: tick.observed_at,
            last_error: (!evaluation.recovered).then(|| evaluation.reason.clone()),
        })
        .await?;
    store
        .record_runtime_worker_observation(&RuntimeWorkerObservation {
            account_id: tick.account_id,
            capability: "worker-crash-recovery".into(),
            worker_kind: "WorkerCrashRecovery".into(),
            status: if evaluation.recovered {
                "Healthy".into()
            } else {
                "Stale".into()
            },
            should_fail_closed: !evaluation.recovered,
            reason: evaluation.reason.clone(),
            observed_at: Some(tick.observed_at),
        })
        .await?;
    Ok(WorkerCrashRecoveryTickReceipt {
        evaluation,
        heartbeat_recorded: true,
        observation_recorded: true,
    })
}
