use chrono::Utc;
use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::{ServiceError, record_runtime_worker_signals};

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
