use crate::backend::AppState;
use crate::model::*;
use crate::support::{ApiResult, require, service_error};
use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use pmx_authz::Operation;
use pmx_core::*;
use pmx_service::{
    StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest, SubmitOutcome,
};

pub(crate) async fn normalize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(intent): Json<TradeIntent>,
) -> ApiResult<NormalizedIntent> {
    require(&headers, Operation::NormalizeIntent)?;
    let normalized = state
        .service
        .normalize(intent)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(normalized)))
}

pub(crate) async fn capture_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(intent): Json<NormalizedIntent>,
) -> ApiResult<FeasibilitySnapshot> {
    require(&headers, Operation::CaptureSnapshot)?;
    let snapshot = state
        .service
        .capture_snapshot(intent)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(snapshot)))
}

pub(crate) async fn decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DecisionRequest>,
) -> ApiResult<ConstraintDecision> {
    require(&headers, Operation::EvaluateDecision)?;
    let decision = state
        .service
        .evaluate_decision_by_id(pmx_service::DecisionByIdRequest {
            normalized_intent_id: req.normalized_intent_id,
            snapshot_id: req.snapshot_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(decision)))
}

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
    let outcome = state
        .service
        .submit_plan(pmx_service::SubmitPlanCommand {
            execution_id: req.execution_id,
            plan_hash: req.plan_hash,
            idempotency_key: req.idempotency_key,
        })
        .await
        .map_err(service_error)?;
    match outcome {
        SubmitOutcome::Accepted(receipt) => Ok((StatusCode::ACCEPTED, Json(receipt))),
        SubmitOutcome::Replayed(receipt) => Ok((StatusCode::OK, Json(receipt))),
    }
}

pub(crate) async fn record_sign_only_lifecycle_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(record): Json<SignOnlyLifecycleRecord>,
) -> ApiResult<SignOnlyLifecycleRecord> {
    require(&headers, Operation::RecordSignOnlyLifecycle)?;
    let recorded = state
        .service
        .record_sign_only_lifecycle_event(record)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::ACCEPTED, Json(recorded)))
}

pub(crate) async fn record_standard_sign_only_construction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<StandardSignOnlyConstructionRequest>,
) -> ApiResult<StandardSignOnlyConstructionReceipt> {
    require(&headers, Operation::RecordSignOnlyLifecycle)?;
    let receipt = state
        .service
        .record_standard_sign_only_construction(req)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}
