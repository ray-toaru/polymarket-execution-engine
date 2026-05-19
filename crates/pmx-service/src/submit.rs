use pmx_core::{
    DecimalString, ExecutionId, OrderReservation, QuantityBound, ReservationState, SubmitReceipt,
    SubmitStatus, canonical_json_sha256,
};
use pmx_store::{
    ExecutionLifecycleEvent, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore,
};
use uuid::Uuid;

use crate::{ServiceError, SubmitOutcome, SubmitPlanCommand};

#[path = "submit/blocked.rs"]
mod blocked;

#[path = "submit/fingerprint.rs"]
mod fingerprint;

#[path = "submit/replay.rs"]
mod replay;

pub async fn submit_plan<S>(
    store: &S,
    req: SubmitPlanCommand,
    executor_version: &str,
    contract_version: &str,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore + IdempotencyStore + ExecutionLifecycleStore + Send + Sync,
{
    let plan = store.load_plan_summary(&req.execution_id).await?;
    if plan.plan_hash.0 != req.plan_hash {
        return Err(ServiceError::Conflict(
            "plan_hash does not match server-authoritative plan".into(),
        ));
    }
    if !matches!(
        plan.status,
        pmx_core::PlanStatus::Ready | pmx_core::PlanStatus::Blocked
    ) {
        return Err(ServiceError::Conflict("plan status is invalid".into()));
    }
    let request_fingerprint = fingerprint::request_fingerprint(&req)?;
    match store
        .begin_submit_attempt(
            &plan.account_id.0,
            &plan.execution_id,
            &req.idempotency_key,
            &request_fingerprint,
        )
        .await?
    {
        IdempotencyAction::ReplayStoredResponse { response_json, .. } => {
            replay::replayed_submit_outcome(&response_json)
        }
        IdempotencyAction::Conflict => Err(ServiceError::Conflict(
            "idempotency key reused with different request fingerprint".into(),
        )),
        IdempotencyAction::InProgress { retry_after_ms, .. } => {
            Err(ServiceError::InProgress { retry_after_ms })
        }
        IdempotencyAction::Proceed { submit_attempt, .. } => {
            blocked::blocked_submit_outcome(
                store,
                &plan,
                &req.idempotency_key,
                &request_fingerprint,
                submit_attempt,
                executor_version,
                contract_version,
            )
            .await
        }
    }
}
