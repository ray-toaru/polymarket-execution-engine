use crate::backend::{AppState, CONTRACT_VERSION};
use crate::model::*;
use crate::support::{
    ApiResult, api_error_with_correlation, correlation_id_from_headers, record_admin_audit,
    request_fingerprint, require, service_error, validate_auth_config_from_env,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::Utc;
use pmx_authz::Operation;
use pmx_core::*;
use pmx_service::{
    StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest, SubmitOutcome,
};
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, ExecutionLifecycleEvent, ExecutionLifecycleQuery,
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, PostgresStore, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusReport, SignOnlyLifecycleQuery,
};
use uuid::Uuid;

fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/intents/normalize", post(normalize))
        .route("/v1/snapshots/capture", post(capture_snapshot))
        .route("/v1/decisions/evaluate", post(decide))
        .route("/v1/plans/compile", post(compile_plan))
        .route("/v1/submissions", post(submit_plan))
        .route("/v1/submissions/:execution_id", get(get_submission))
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
            get(list_sign_only_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/executions/:execution_id/events",
            get(list_execution_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/orders/:order_id/events",
            get(list_order_lifecycle_events),
        )
        .route("/v1/runtime/workers", get(list_runtime_worker_status))
        .route("/v1/admin/audit-events", get(list_admin_audit_events))
        .route("/v1/admin/kill-switch", post(set_kill_switch))
        .route("/v1/admin/cancel-order", post(cancel_order_placeholder))
        .route("/v1/admin/reconcile", post(reconcile_placeholder))
        .route(
            "/v1/admin/reconcile-order-local",
            post(reconcile_order_local),
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

async fn get_submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
) -> ApiResult<SubmitReceipt> {
    require(&headers, Operation::ReadReport)?;
    let receipt = state
        .service
        .load_submit_receipt(&execution_id)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(receipt)))
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

async fn list_sign_only_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<SignOnlyLifecycleRecord>> {
    require(&headers, Operation::ReadReport)?;
    let records = state
        .service
        .list_sign_only_lifecycle_events(SignOnlyLifecycleQuery {
            execution_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(records)))
}

async fn list_execution_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<ExecutionLifecycleEvent>> {
    require(&headers, Operation::ReadReport)?;
    let events = state
        .service
        .list_execution_lifecycle_events(ExecutionLifecycleQuery {
            execution_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(events)))
}

async fn list_order_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<OrderLifecycleEventRecord>> {
    require(&headers, Operation::ReadReport)?;
    let events = state
        .service
        .list_order_lifecycle_events(OrderLifecycleEventQuery {
            order_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(events)))
}

async fn list_runtime_worker_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RuntimeWorkerStatusListQuery>,
) -> ApiResult<RuntimeWorkerStatusReport> {
    require(&headers, Operation::ReadReport)?;
    let report = state
        .service
        .list_runtime_worker_status(RuntimeWorkerStatusQuery {
            account_id: query.account_id,
            limit: query.limit.unwrap_or(100),
            before_observed_at: query.before_observed_at,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(report)))
}

async fn list_admin_audit_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> ApiResult<Vec<AdminAuditEvent>> {
    require(&headers, Operation::ReadAudit)?;
    let events = state
        .service
        .list_admin_audit_events(AdminAuditQuery {
            limit: query.limit.unwrap_or(100),
            before_audit_id: query.before_audit_id,
            operation: query.operation,
            principal_subject: query.principal_subject,
            result: query.result,
            correlation_id: query.correlation_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(events)))
}

async fn set_kill_switch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KillSwitchRequest>,
) -> ApiResult<KillSwitchReceipt> {
    let principal = require(&headers, Operation::KillSwitch)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let fingerprint = request_fingerprint(&req);
    let receipt = KillSwitchReceipt {
        enabled: req.enabled,
        changed_at: Utc::now(),
        reason: req.reason,
    };
    record_admin_audit(
        &state,
        &principal,
        "KillSwitch",
        fingerprint,
        Some(correlation_id.clone()),
        format!(
            "ACCEPTED enabled={} correlation_id={}",
            receipt.enabled, correlation_id
        ),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}

async fn cancel_order_placeholder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CancelOrderRequest>,
) -> ApiResult<CancelReceipt> {
    let principal = require(&headers, Operation::CancelOrder)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let fingerprint = request_fingerprint(&req);
    if req.account_id.trim().is_empty()
        || req.order_id.trim().is_empty()
        || req.reason.trim().is_empty()
    {
        record_admin_audit(
            &state,
            &principal,
            "CancelOrder",
            fingerprint,
            Some(correlation_id.clone()),
            "REJECTED bad_request",
        )
        .await?;
        return Err(api_error_with_correlation(
            StatusCode::BAD_REQUEST,
            "account_id, order_id and reason must be non-empty",
            correlation_id,
        ));
    }
    let fingerprint = request_fingerprint(&req);
    let reason_len = req.reason.len() + req.account_id.len();
    let execution_id = req.execution_id.clone();
    let order_id = req.order_id.clone();
    let receipt = CancelReceipt {
        cancel_id: format!("cancel-{}-{}", reason_len, Uuid::new_v4()),
        order_id: req.order_id,
        state: CancelState::ReconcileRequired,
    };
    let order_lifecycle = state
        .service
        .record_non_live_cancel_request(&order_id, &req.reason, Some(correlation_id.clone()))
        .await
        .map_err(service_error)?;
    if let Some(execution_id) = execution_id {
        state
            .service
            .record_execution_lifecycle_event(ExecutionLifecycleEvent {
                event_id: None,
                execution_id,
                account_id: req.account_id.clone(),
                event_type: "CANCEL_REQUESTED_NON_LIVE".into(),
                event_source: "pmx-api".into(),
                payload: redacted_payload_envelope(
                    "cancel_requested_non_live",
                    Some(correlation_id.clone()),
                    serde_json::json!({
                        "cancel_id": receipt.cancel_id.clone(),
                        "order_id": order_id,
                        "cancel_state": format!("{:?}", receipt.state),
                        "order_lifecycle_state": order_lifecycle
                            .as_ref()
                            .map(|order| format!("{:?}", order.lifecycle_state)),
                        "no_remote_side_effect": true,
                    }),
                ),
                created_at: None,
            })
            .await
            .map_err(service_error)?;
    }
    record_admin_audit(
        &state,
        &principal,
        "CancelOrder",
        fingerprint,
        Some(correlation_id.clone()),
        format!(
            "ACCEPTED state={:?} correlation_id={}",
            receipt.state, correlation_id
        ),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}

async fn reconcile_placeholder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReconcileRequest>,
) -> ApiResult<ReconcileReport> {
    let principal = require(&headers, Operation::Reconcile)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let fingerprint = request_fingerprint(&req);
    if req.reason.trim().is_empty() {
        record_admin_audit(
            &state,
            &principal,
            "Reconcile",
            fingerprint,
            Some(correlation_id.clone()),
            "REJECTED bad_request",
        )
        .await?;
        return Err(api_error_with_correlation(
            StatusCode::BAD_REQUEST,
            "reason must be non-empty",
            correlation_id,
        ));
    }
    let local_reconcile = match (&req.order_id, &req.remote_observation) {
        (Some(order_id), Some(remote_observation)) => {
            Some((order_id.clone(), remote_observation.clone()))
        }
        (None, None) => None,
        _ => {
            record_admin_audit(
                &state,
                &principal,
                "Reconcile",
                fingerprint,
                Some(correlation_id.clone()),
                "REJECTED bad_request",
            )
            .await?;
            return Err(api_error_with_correlation(
                StatusCode::BAD_REQUEST,
                "order_id and remote_observation must be provided together",
                correlation_id,
            ));
        }
    };
    let fingerprint = request_fingerprint(&req);
    let execution_id = req.execution_id.clone();
    let mut report = ReconcileReport {
        reconcile_id: format!("reconcile-{}", Uuid::new_v4()),
        status: "SCHEDULED_STATE_MACHINE_REQUIRED".into(),
        checked_orders: 0,
        findings: vec![
            format!("account_id={}", req.account_id.0),
            req.reason.clone(),
        ],
    };
    if let Some((order_id, remote_observation)) = local_reconcile {
        let Some((divergence, updated_order)) = state
            .service
            .reconcile_order_lifecycle_divergence(
                &order_id,
                Some(&req.account_id.0),
                remote_observation,
                &req.reason,
                Some(correlation_id.clone()),
            )
            .await
            .map_err(service_error)?
        else {
            record_admin_audit(
                &state,
                &principal,
                "Reconcile",
                fingerprint,
                Some(correlation_id.clone()),
                "REJECTED missing_order",
            )
            .await?;
            return Err(api_error_with_correlation(
                StatusCode::NOT_FOUND,
                "order lifecycle not found",
                correlation_id,
            ));
        };
        report.checked_orders = 1;
        report.status = if divergence.operator_required {
            "OPERATOR_REQUIRED_NON_LIVE".into()
        } else {
            "LOCAL_RECONCILE_RECORDED_NON_LIVE".into()
        };
        report.findings.push(format!("order_id={order_id}"));
        report
            .findings
            .push(format!("divergence={:?}", divergence.kind));
        if let Some(updated_order) = updated_order {
            report
                .findings
                .push(format!("local_state={:?}", updated_order.lifecycle_state));
        }
    }
    if let Some(execution_id) = execution_id {
        state
            .service
            .record_execution_lifecycle_event(ExecutionLifecycleEvent {
                event_id: None,
                execution_id,
                account_id: req.account_id.0.clone(),
                event_type: "RECONCILE_REQUESTED_NON_LIVE".into(),
                event_source: "pmx-api".into(),
                payload: redacted_payload_envelope(
                    "reconcile_requested_non_live",
                    Some(correlation_id.clone()),
                    serde_json::json!({
                        "reconcile_id": report.reconcile_id.clone(),
                        "status": report.status.clone(),
                        "no_remote_side_effect": true,
                    }),
                ),
                created_at: None,
            })
            .await
            .map_err(service_error)?;
    }
    record_admin_audit(
        &state,
        &principal,
        "Reconcile",
        fingerprint,
        Some(correlation_id.clone()),
        format!(
            "ACCEPTED status={} correlation_id={}",
            report.status, correlation_id
        ),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(report)))
}

async fn reconcile_order_local(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReconcileOrderLocalRequest>,
) -> ApiResult<ReconcileOrderLocalResponse> {
    let principal = require(&headers, Operation::Reconcile)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let fingerprint = request_fingerprint(&req);
    if req.account_id.trim().is_empty()
        || req.order_id.trim().is_empty()
        || req.reason.trim().is_empty()
    {
        record_admin_audit(
            &state,
            &principal,
            "ReconcileOrderLocal",
            fingerprint,
            Some(correlation_id.clone()),
            "REJECTED bad_request",
        )
        .await?;
        return Err(api_error_with_correlation(
            StatusCode::BAD_REQUEST,
            "account_id, order_id and reason must be non-empty",
            correlation_id,
        ));
    }
    let Some((divergence, updated_order)) = state
        .service
        .reconcile_order_lifecycle_divergence(
            &req.order_id,
            Some(&req.account_id),
            req.remote_observation,
            &req.reason,
            Some(correlation_id.clone()),
        )
        .await
        .map_err(service_error)?
    else {
        record_admin_audit(
            &state,
            &principal,
            "ReconcileOrderLocal",
            fingerprint,
            Some(correlation_id.clone()),
            "REJECTED missing_order",
        )
        .await?;
        return Err(api_error_with_correlation(
            StatusCode::NOT_FOUND,
            "order lifecycle not found",
            correlation_id,
        ));
    };
    record_admin_audit(
        &state,
        &principal,
        "ReconcileOrderLocal",
        fingerprint,
        Some(correlation_id.clone()),
        format!(
            "ACCEPTED kind={:?} correlation_id={}",
            divergence.kind, correlation_id
        ),
    )
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(ReconcileOrderLocalResponse {
            order_id: req.order_id,
            divergence,
            updated_order,
            no_remote_side_effect: true,
        }),
    ))
}
