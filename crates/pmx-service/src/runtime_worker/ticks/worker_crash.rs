use pmx_runtime::{WorkerCrashRecoveryEvaluationInput, evaluate_worker_crash_recovery};
use pmx_store::{
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore,
};

use crate::model::*;

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
