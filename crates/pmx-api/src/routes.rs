use crate::backend::{AppState, CONTRACT_VERSION};
use crate::model::*;
use crate::support::{ApiResult, require, service_error, validate_auth_config_from_env};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use pmx_authz::Operation;
use pmx_core::*;
use pmx_service::{
    StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest, SubmitOutcome,
};
use pmx_store::PostgresStore;

mod admin;
mod read;

fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/intents/normalize", post(normalize))
        .route("/v1/snapshots/capture", post(capture_snapshot))
        .route("/v1/decisions/evaluate", post(decide))
        .route("/v1/plans/compile", post(compile_plan))
        .route("/v1/submissions", post(submit_plan))
        .route("/v1/submissions/:execution_id", get(read::get_submission))
        .route(
            "/v1/sign-only/lifecycle-events",
            post(record_sign_only_lifecycle_event),
        )
        .route(
            "/v1/sign-only/standard-constructions",
            post(record_standard_sign_only_construction),
        )
        .route(
            "/v1/sign-only/lifecycle-events/:execution_id",
            get(read::list_sign_only_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/executions/:execution_id/events",
            get(read::list_execution_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/orders/:order_id/events",
            get(read::list_order_lifecycle_events),
        )
        .route("/v1/runtime/workers", get(read::list_runtime_worker_status))
        .route(
            "/v1/admin/audit-events",
            get(admin::list_admin_audit_events),
        )
        .route("/v1/admin/kill-switch", post(admin::set_kill_switch))
        .route(
            "/v1/admin/cancel-order",
            post(admin::cancel_order_placeholder),
        )
        .route("/v1/admin/reconcile", post(admin::reconcile_placeholder))
        .route(
            "/v1/admin/reconcile-order-local",
            post(admin::reconcile_order_local),
        )
        .with_state(state)
}

pub fn try_app() -> Result<Router, String> {
    validate_auth_config_from_env()?;
    Ok(router_with_state(AppState::default()))
}

pub fn app() -> Router {
    try_app().expect("PM_EXEC_SERVICE_TOKEN and PM_EXEC_ADMIN_TOKEN must be non-empty and distinct")
}

/// Build an HTTP API backed by a PostgreSQL store.
///
/// This helper is intended for integration tests and non-live smoke environments. It applies the
/// schema only when requested by the caller. The resulting API still blocks live submit; it only
/// proves the server-authoritative object graph and submit receipt path against PostgreSQL.
pub async fn try_postgres_app(
    database_url: impl Into<String>,
    apply_schema: bool,
) -> Result<Router, String> {
    validate_auth_config_from_env()?;
    let store = PostgresStore::connect(database_url.into())
        .await
        .map_err(|err| format!("postgres connect failed: {err}"))?;
    if apply_schema {
        store
            .apply_schema()
            .await
            .map_err(|err| format!("postgres schema apply failed: {err}"))?;
    }
    Ok(router_with_state(AppState::postgres(store)))
}

async fn health(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<serde_json::Value> {
    require(&headers, Operation::ReadReport)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "NOT_READY",
            "executor_version": env!("CARGO_PKG_VERSION"),
            "contract_version": CONTRACT_VERSION,
            "checks": {
                "live_gateway": "not_configured",
                "database": state.service.storage_mode(),
                "signer": "not_configured",
                "auth": "enabled_distinct_tokens",
                "service_layer": "pmx_service_server_authoritative_id_bound_admin_audit"
            }
        })),
    ))
}

async fn normalize(
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

async fn capture_snapshot(
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

async fn decide(
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

async fn compile_plan(
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

async fn submit_plan(
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

async fn record_sign_only_lifecycle_event(
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

async fn record_standard_sign_only_construction(
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
