use super::*;
use crate::support::correlation_id_from_headers;

pub(crate) async fn compile_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CompilePlanRequest>,
) -> ApiResult<ExecutionPlanSummary> {
    require(&headers, Operation::CompilePlan)?;
    let plan = state
        .service
        .compile_plan_by_id(pmx_service::CompilePlanByIdCommand {
            normalized_intent_id: req.normalized_intent_id,
            snapshot_id: req.snapshot_id,
            decision_id: req.decision_id,
            approval: req.approval,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(plan)))
}

pub(crate) async fn submit_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SubmitPlanRequest>,
) -> ApiResult<SubmitReceipt> {
    require(&headers, Operation::SubmitPlan)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let outcome = state
        .service
        .submit_plan(pmx_service::SubmitPlanCommand {
            execution_id: req.execution_id,
            plan_hash: req.plan_hash,
            idempotency_key: req.idempotency_key,
            mode: req.mode,
            correlation_id: Some(correlation_id),
        })
        .await
        .map_err(service_error)?;
    match outcome {
        SubmitOutcome::Accepted(receipt) => Ok((StatusCode::ACCEPTED, Json(receipt))),
        SubmitOutcome::Replayed(receipt) => Ok((StatusCode::OK, Json(receipt))),
    }
}
