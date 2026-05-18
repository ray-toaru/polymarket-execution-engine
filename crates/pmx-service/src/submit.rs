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
    let request_fingerprint = canonical_json_sha256(&req)
        .map_err(|err| ServiceError::Internal(err.to_string()))?
        .0;
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
            let receipt: SubmitReceipt = serde_json::from_str(&response_json).map_err(|err| {
                ServiceError::Internal(format!("stored submit receipt is invalid: {err}"))
            })?;
            Ok(SubmitOutcome::Replayed(receipt))
        }
        IdempotencyAction::Conflict => Err(ServiceError::Conflict(
            "idempotency key reused with different request fingerprint".into(),
        )),
        IdempotencyAction::InProgress { retry_after_ms, .. } => {
            Err(ServiceError::InProgress { retry_after_ms })
        }
        IdempotencyAction::Proceed { submit_attempt, .. } => {
            if matches!(plan.status, pmx_core::PlanStatus::Ready) {
                let reservation = OrderReservation {
                    reservation_id: format!("res-{}-{submit_attempt}", plan.execution_id),
                    account_id: plan.account_id.clone(),
                    execution_id: ExecutionId(plan.execution_id.clone()),
                    internal_order_id: None,
                    quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString(
                        "0.00000001".into(),
                    )),
                    state: ReservationState::Pending,
                };
                store.save_order_reservation(&reservation).await?;
            }
            let receipt = SubmitReceipt {
                execution_id: req.execution_id,
                receipt_id: format!("receipt-blocked-{submit_attempt}-{}", Uuid::new_v4()),
                status: SubmitStatus::Blocked,
                executor_version: executor_version.to_owned(),
                contract_version: contract_version.to_owned(),
            };
            let response_json = serde_json::to_string(&receipt).map_err(|err| {
                ServiceError::Internal(format!("submit receipt serialization failed: {err}"))
            })?;
            let response_fingerprint = canonical_json_sha256(&receipt)
                .map_err(|err| ServiceError::Internal(err.to_string()))?
                .0;
            store
                .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                    event_id: None,
                    execution_id: plan.execution_id.clone(),
                    account_id: plan.account_id.0.clone(),
                    event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
                    event_source: "pmx-service".into(),
                    payload: serde_json::json!({
                        "submit_attempt": submit_attempt,
                        "plan_status": format!("{:?}", plan.status),
                        "no_remote_side_effect": true,
                        "receipt_id": receipt.receipt_id.clone(),
                    }),
                    created_at: None,
                })
                .await?;
            store.record_submit_receipt(&receipt).await?;
            store
                .finish_submit_attempt(
                    &plan.account_id.0,
                    &plan.execution_id,
                    &req.idempotency_key,
                    &request_fingerprint,
                    &response_fingerprint,
                    &response_json,
                )
                .await?;
            Ok(SubmitOutcome::Accepted(receipt))
        }
    }
}
