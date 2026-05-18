use chrono::Utc;
use pmx_runtime::{
    RuntimeSignal, RuntimeWorkerProviderSnapshot, runtime_worker_loop_tick,
    runtime_worker_store_writes,
};
use pmx_store::{
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore,
};

use crate::model::*;

mod ticks;
pub use ticks::*;

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
