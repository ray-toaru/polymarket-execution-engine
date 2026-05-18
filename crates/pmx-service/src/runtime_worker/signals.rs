use pmx_runtime::{RuntimeSignal, runtime_worker_store_writes};
use pmx_store::{RuntimeWorkerObservation, RuntimeWorkerObservationStore};

use crate::ServiceError;

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
