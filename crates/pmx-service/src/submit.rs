use pmx_core::{SubmitReceipt, SubmitStatus, canonical_json_sha256};
use pmx_gateway::{ClobGateway, SignerProvider};
use pmx_store::{
    ExecutionLifecycleEvent, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore, OrderLifecycleStore, OrderReconcileBacklogStore,
};
use uuid::Uuid;

use crate::{RuntimeStateProvider, ServiceError, SubmitMode, SubmitOutcome, SubmitPlanCommand};

#[path = "submit/blocked.rs"]
mod blocked;

#[path = "submit/fingerprint.rs"]
mod fingerprint;

#[path = "submit/live.rs"]
mod live;

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
    if matches!(req.mode, SubmitMode::Live) {
        return Err(ServiceError::Conflict(
            "LIVE submit mode is fail-closed until gateway posting is wired through the executor service".into(),
        ));
    }
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
        IdempotencyAction::Proceed {
            submit_attempt,
            owner_token,
        } => {
            blocked::blocked_submit_outcome(
                store,
                blocked::BlockedSubmitRequest {
                    plan: &plan,
                    idempotency_key: &req.idempotency_key,
                    request_fingerprint: &request_fingerprint,
                    submit_attempt,
                    owner_token: &owner_token,
                    executor_version,
                    contract_version,
                    correlation_id: req.correlation_id.as_deref(),
                },
            )
            .await
        }
    }
}

pub async fn submit_plan_with_gateway<S, R, P, G>(
    store: &S,
    runtime_state_provider: &R,
    signer_provider: &P,
    gateway: &G,
    req: SubmitPlanCommand,
    executor_version: &str,
    contract_version: &str,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore
        + IdempotencyStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + OrderReconcileBacklogStore
        + Send
        + Sync,
    R: RuntimeStateProvider,
    P: SignerProvider,
    G: ClobGateway,
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
        IdempotencyAction::Proceed {
            submit_attempt,
            owner_token,
        } => match req.mode {
            SubmitMode::BlockedDryRun => {
                blocked::blocked_submit_outcome(
                    store,
                    blocked::BlockedSubmitRequest {
                        plan: &plan,
                        idempotency_key: &req.idempotency_key,
                        request_fingerprint: &request_fingerprint,
                        submit_attempt,
                        owner_token: &owner_token,
                        executor_version,
                        contract_version,
                        correlation_id: req.correlation_id.as_deref(),
                    },
                )
                .await
            }
            SubmitMode::Live => {
                live::live_submit_outcome(
                    store,
                    runtime_state_provider,
                    signer_provider,
                    gateway,
                    live::LiveSubmitRequest {
                        plan: &plan,
                        idempotency_key: &req.idempotency_key,
                        request_fingerprint: &request_fingerprint,
                        submit_attempt,
                        owner_token: &owner_token,
                        executor_version,
                        contract_version,
                        correlation_id: req.correlation_id.as_deref(),
                    },
                )
                .await
            }
        },
    }
}
