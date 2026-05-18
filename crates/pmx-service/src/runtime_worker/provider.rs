use pmx_runtime::{RuntimeWorkerProviderSnapshot, runtime_worker_loop_tick};
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::{ServiceError, record_runtime_worker_tick};

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
