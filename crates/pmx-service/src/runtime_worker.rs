use chrono::Utc;
use pmx_core::*;
use pmx_runtime::{
    GeoblockEvaluationInput, HeartbeatLeaseElectionInput, ReconcileBacklogEvaluationInput,
    ResourceRefreshEvaluationInput, RuntimeSignal, RuntimeWorkerProviderSnapshot,
    WebSocketLivenessEvaluationInput, WorkerCrashRecoveryEvaluationInput,
    elect_heartbeat_lease_owner, evaluate_geoblock_status, evaluate_reconcile_backlog,
    evaluate_resource_refresh_freshness, evaluate_websocket_liveness,
    evaluate_worker_crash_recovery, runtime_worker_loop_tick, runtime_worker_store_writes,
};
use pmx_store::{
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, RuntimeWorkerHealthStore,
    RuntimeWorkerHeartbeat, RuntimeWorkerObservation, RuntimeWorkerObservationStore,
};

use crate::model::*;

pub async fn record_runtime_worker_signals<S>(
    store: &S,
    account_id: impl Into<String>,
    signals: &[RuntimeSignal],
) -> Result<usize, ServiceError>
where
    S: RuntimeWorkerObservationStore + Send + Sync,
{
    let writes = runtime_worker_store_writes(account_id, signals);
    for write in &writes {
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: write.account_id.clone(),
                capability: write.capability.clone(),
                worker_kind: format!("{:?}", write.worker_kind),
                status: format!("{:?}", write.status),
                should_fail_closed: write.should_fail_closed,
                reason: write.reason.clone(),
                observed_at: None,
            })
            .await?;
    }
    Ok(writes.len())
}

pub async fn record_runtime_worker_tick<S>(
    store: &S,
    account_id: impl Into<String>,
    tick: RuntimeWorkerTick,
) -> Result<RuntimeWorkerTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.worker_id.trim().is_empty()
        || tick.role.trim().is_empty()
        || tick.capability.trim().is_empty()
        || tick.status.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "worker_id, role, capability and status must be non-empty".into(),
        ));
    }
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: tick.worker_id.clone(),
            role: tick.role.clone(),
            capability: tick.capability.clone(),
            status: tick.status.clone(),
            last_heartbeat_at: Utc::now(),
            last_error: tick.last_error.clone(),
        })
        .await?;
    let observations_recorded =
        record_runtime_worker_signals(store, account_id, &tick.signals).await?;
    Ok(RuntimeWorkerTickReceipt {
        worker_id: tick.worker_id,
        capability: tick.capability,
        heartbeat_recorded: true,
        observations_recorded,
    })
}

pub async fn record_runtime_worker_provider_snapshot<S>(
    store: &S,
    snapshot: RuntimeWorkerProviderSnapshot,
) -> Result<RuntimeWorkerProviderTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if snapshot.provider_name.trim().is_empty()
        || snapshot.instance_id.trim().is_empty()
        || snapshot.account_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "provider_name, instance_id and account_id must be non-empty".into(),
        ));
    }
    if !snapshot.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "runtime worker provider snapshots must not contain trading side effects".into(),
        ));
    }
    let account_id = snapshot.account_id.clone();
    let provider_name = snapshot.provider_name.clone();
    let instance_id = snapshot.instance_id.clone();
    let tick = runtime_worker_loop_tick(snapshot.into_loop_input());
    let status = if tick.submit_allowed_by_runtime {
        "HEALTHY"
    } else {
        "DEGRADED"
    };
    let receipt = record_runtime_worker_tick(
        store,
        account_id,
        RuntimeWorkerTick {
            worker_id: instance_id.clone(),
            role: provider_name.clone(),
            capability: "runtime-worker-loop".into(),
            status: status.into(),
            last_error: (!tick.submit_allowed_by_runtime)
                .then(|| "runtime worker loop fail-closed".into()),
            signals: tick.signals,
        },
    )
    .await?;
    Ok(RuntimeWorkerProviderTickReceipt {
        worker_id: instance_id,
        provider_name,
        lease_owner_active: tick.lease_owner_active,
        submit_allowed_by_runtime: tick.submit_allowed_by_runtime,
        heartbeat_recorded: receipt.heartbeat_recorded,
        observations_recorded: receipt.observations_recorded,
    })
}

pub async fn record_runtime_worker_continuous_tick<S>(
    store: &S,
    tick: RuntimeWorkerContinuousTick,
) -> Result<RuntimeWorkerContinuousTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "runtime worker continuous ticks must not contain trading side effects".into(),
        ));
    }
    if tick.snapshots.is_empty() {
        return Err(ServiceError::BadRequest(
            "runtime worker continuous ticks require at least one snapshot".into(),
        ));
    }

    let mut ticks_recorded = Vec::with_capacity(tick.snapshots.len());
    for snapshot in tick.snapshots {
        if !snapshot.no_trading_side_effect {
            return Err(ServiceError::BadRequest(
                "runtime worker provider snapshots must not contain trading side effects".into(),
            ));
        }
        ticks_recorded.push(record_runtime_worker_provider_snapshot(store, snapshot).await?);
    }
    let all_submit_allowed_by_runtime = ticks_recorded
        .iter()
        .all(|receipt| receipt.submit_allowed_by_runtime);

    Ok(RuntimeWorkerContinuousTickReceipt {
        ticks_recorded,
        all_submit_allowed_by_runtime,
        no_trading_side_effect: true,
    })
}

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
