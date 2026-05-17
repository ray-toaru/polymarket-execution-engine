use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    routing::{get, post},
};
use chrono::Utc;
use pmx_authz::{Operation, Principal, Scope, authorize};
use pmx_core::*;
use pmx_service::{ExecutorService, ServiceError, StoreBackedRuntimeStateProvider, SubmitOutcome};
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, ExecutionLifecycleEvent, ExecutionLifecycleQuery,
    InMemoryStore, PostgresStore, SignOnlyLifecycleQuery, StoreError,
};
use uuid::Uuid;

const CONTRACT_VERSION: &str = "1.0.0-draft";

#[derive(Clone)]
pub enum ServiceBackend {
    InMemory(ExecutorService<InMemoryStore>),
    Postgres(ExecutorService<PostgresStore, StoreBackedRuntimeStateProvider<PostgresStore>>),
}

impl ServiceBackend {
    fn storage_mode(&self) -> &'static str {
        match self {
            Self::InMemory(_) => "in_memory_scaffold",
            Self::Postgres(_) => "postgres",
        }
    }

    async fn normalize(&self, intent: TradeIntent) -> Result<NormalizedIntent, ServiceError> {
        match self {
            Self::InMemory(service) => service.normalize(intent).await,
            Self::Postgres(service) => service.normalize(intent).await,
        }
    }

    async fn capture_snapshot(
        &self,
        normalized: NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        match self {
            Self::InMemory(service) => service.capture_snapshot(normalized).await,
            Self::Postgres(service) => service.capture_snapshot(normalized).await,
        }
    }

    async fn evaluate_decision_by_id(
        &self,
        req: pmx_service::DecisionByIdRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        match self {
            Self::InMemory(service) => service.evaluate_decision_by_id(req).await,
            Self::Postgres(service) => service.evaluate_decision_by_id(req).await,
        }
    }

    async fn compile_plan_by_id(
        &self,
        req: pmx_service::CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        match self {
            Self::InMemory(service) => service.compile_plan_by_id(req).await,
            Self::Postgres(service) => service.compile_plan_by_id(req).await,
        }
    }

    async fn submit_plan(
        &self,
        req: pmx_service::SubmitPlanCommand,
    ) -> Result<SubmitOutcome, ServiceError> {
        match self {
            Self::InMemory(service) => service.submit_plan(req).await,
            Self::Postgres(service) => service.submit_plan(req).await,
        }
    }

    async fn record_admin_audit_event(&self, event: AdminAuditEvent) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_admin_audit_event(event).await,
            Self::Postgres(service) => service.record_admin_audit_event(event).await,
        }
    }

    async fn list_admin_audit_events(
        &self,
        query: AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_admin_audit_events(query).await,
            Self::Postgres(service) => service.list_admin_audit_events(query).await,
        }
    }

    async fn record_execution_lifecycle_event(
        &self,
        event: ExecutionLifecycleEvent,
    ) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_execution_lifecycle_event(event).await,
            Self::Postgres(service) => service.record_execution_lifecycle_event(event).await,
        }
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_execution_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_execution_lifecycle_events(query).await,
        }
    }

    async fn record_sign_only_lifecycle_event(
        &self,
        record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        match self {
            Self::InMemory(service) => service.record_sign_only_lifecycle_event(record).await,
            Self::Postgres(service) => service.record_sign_only_lifecycle_event(record).await,
        }
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_sign_only_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_sign_only_lifecycle_events(query).await,
        }
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, ServiceError> {
        match self {
            Self::InMemory(service) => service.load_submit_receipt(execution_id).await,
            Self::Postgres(service) => service.load_submit_receipt(execution_id).await,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    service: ServiceBackend,
}

impl AppState {
    pub fn in_memory() -> Self {
        Self {
            service: ServiceBackend::InMemory(ExecutorService::new(InMemoryStore::default())),
        }
    }

    pub fn postgres(store: PostgresStore) -> Self {
        let provider = StoreBackedRuntimeStateProvider::new(store.clone());
        Self {
            service: ServiceBackend::Postgres(ExecutorService::with_runtime_provider(
                store,
                provider,
                env!("CARGO_PKG_VERSION").to_owned(),
                CONTRACT_VERSION.to_owned(),
            )),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::in_memory()
    }
}

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
            "/v1/sign-only/lifecycle-events/:execution_id",
            get(list_sign_only_lifecycle_events),
        )
        .route(
            "/v1/lifecycle/executions/:execution_id/events",
            get(list_execution_lifecycle_events),
        )
        .route("/v1/admin/audit-events", get(list_admin_audit_events))
        .route("/v1/admin/kill-switch", post(set_kill_switch))
        .route("/v1/admin/cancel-order", post(cancel_order_placeholder))
        .route("/v1/admin/reconcile", post(reconcile_placeholder))
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

type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<serde_json::Value>)>;

fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

fn api_error_with_correlation(
    status: StatusCode,
    message: impl Into<String>,
    correlation_id: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": message.into(),
            "correlation_id": correlation_id.into(),
        })),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenConfig {
    pub service_token: String,
    pub admin_token: String,
}

pub fn validate_auth_config_from_env() -> Result<AuthTokenConfig, String> {
    let admin_token = std::env::var("PM_EXEC_ADMIN_TOKEN").unwrap_or_default();
    let service_token = std::env::var("PM_EXEC_SERVICE_TOKEN").unwrap_or_default();
    if service_token.is_empty() {
        return Err("PM_EXEC_SERVICE_TOKEN must be set".into());
    }
    if admin_token.is_empty() {
        return Err("PM_EXEC_ADMIN_TOKEN must be set".into());
    }
    if service_token == admin_token {
        return Err("PM_EXEC_SERVICE_TOKEN and PM_EXEC_ADMIN_TOKEN must be distinct".into());
    }
    Ok(AuthTokenConfig {
        service_token,
        admin_token,
    })
}

fn principal_from_headers(
    headers: &HeaderMap,
) -> Result<Principal, (StatusCode, Json<serde_json::Value>)> {
    let auth_config = validate_auth_config_from_env().map_err(|err| {
        api_error_with_correlation(
            StatusCode::INTERNAL_SERVER_ERROR,
            err,
            correlation_id_from_headers(headers),
        )
    })?;
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            api_error_with_correlation(
                StatusCode::UNAUTHORIZED,
                "missing Authorization bearer token",
                correlation_id_from_headers(headers),
            )
        })?;
    let Some(token) = header.strip_prefix("Bearer ") else {
        return Err(api_error_with_correlation(
            StatusCode::UNAUTHORIZED,
            "Authorization must use Bearer token",
            correlation_id_from_headers(headers),
        ));
    };

    if token == auth_config.admin_token {
        return Ok(Principal {
            subject: "admin-token".into(),
            scopes: vec![Scope::Admin],
        });
    }
    if token == auth_config.service_token {
        return Ok(Principal {
            subject: "service-token".into(),
            scopes: vec![Scope::Service],
        });
    }
    Err(api_error_with_correlation(
        StatusCode::FORBIDDEN,
        "token is not authorized",
        correlation_id_from_headers(headers),
    ))
}

fn require(
    headers: &HeaderMap,
    op: Operation,
) -> Result<Principal, (StatusCode, Json<serde_json::Value>)> {
    let principal = principal_from_headers(headers)?;
    authorize(&principal, op).map_err(|err| {
        api_error_with_correlation(
            StatusCode::FORBIDDEN,
            err.to_string(),
            correlation_id_from_headers(headers),
        )
    })?;
    Ok(principal)
}

fn service_error(err: ServiceError) -> (StatusCode, Json<serde_json::Value>) {
    match err {
        ServiceError::BadRequest(msg) => api_error(StatusCode::BAD_REQUEST, msg),
        ServiceError::Conflict(msg) => api_error(StatusCode::CONFLICT, msg),
        ServiceError::InProgress { retry_after_ms } => api_error(
            StatusCode::CONFLICT,
            format!("submit attempt already in progress; retry_after_ms={retry_after_ms}"),
        ),
        ServiceError::Store(StoreError::NotFound(msg)) => api_error(StatusCode::NOT_FOUND, msg),
        ServiceError::Store(StoreError::Conflict(msg)) => api_error(StatusCode::CONFLICT, msg),
        ServiceError::Store(other) => {
            api_error(StatusCode::INTERNAL_SERVER_ERROR, other.to_string())
        }
        ServiceError::Internal(msg) => api_error(StatusCode::INTERNAL_SERVER_ERROR, msg),
    }
}

fn correlation_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn request_fingerprint<T: serde::Serialize>(request: &T) -> Option<String> {
    canonical_json_sha256(request).ok().map(|hash| hash.0)
}

async fn record_admin_audit(
    state: &AppState,
    principal: &Principal,
    operation: &'static str,
    request_fingerprint: Option<String>,
    correlation_id: Option<String>,
    result: impl Into<String>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    state
        .service
        .record_admin_audit_event(AdminAuditEvent {
            audit_id: None,
            principal_subject: principal.subject.clone(),
            operation: operation.into(),
            request_fingerprint,
            correlation_id,
            result: result.into(),
            created_at: None,
        })
        .await
        .map_err(service_error)
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub approval: ApprovalReceipt,
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlanRequest {
    pub execution_id: String,
    pub plan_hash: String,
    pub idempotency_key: String,
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventListQuery {
    pub limit: Option<usize>,
    pub before_event_id: Option<i64>,
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditQuery {
    pub limit: Option<usize>,
    pub before_audit_id: Option<i64>,
    pub operation: Option<String>,
    pub principal_subject: Option<String>,
    pub result: Option<String>,
    pub correlation_id: Option<String>,
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CancelOrderRequest {
    pub account_id: String,
    pub order_id: String,
    pub execution_id: Option<String>,
    pub reason: String,
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
    let fingerprint = request_fingerprint(&req);
    let execution_id = req.execution_id.clone();
    let report = ReconcileReport {
        reconcile_id: format!("reconcile-{}", Uuid::new_v4()),
        status: "SCHEDULED_STATE_MACHINE_REQUIRED".into(),
        checked_orders: 0,
        findings: vec![
            format!("account_id={}", req.account_id.0),
            req.reason.clone(),
        ],
    };
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
