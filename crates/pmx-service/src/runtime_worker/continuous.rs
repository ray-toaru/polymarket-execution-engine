use pmx_store::{RuntimeWorkerHealthStore, RuntimeWorkerObservationStore};

use crate::model::*;
use crate::{ServiceError, record_runtime_worker_provider_snapshot};

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
