use chrono::{DateTime, Utc};
use pmx_core::{ApprovalReceipt, OrderLifecycleDivergence, RemoteOrderObservation};
use pmx_store::OrderLifecycleRecord;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub approval: ApprovalReceipt,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlanRequest {
    pub execution_id: String,
    pub plan_hash: String,
    pub idempotency_key: String,
    pub mode: pmx_service::SubmitMode,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventListQuery {
    pub limit: Option<usize>,
    pub before_event_id: Option<i64>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerStatusListQuery {
    pub account_id: String,
    pub limit: Option<usize>,
    pub before_observed_at: Option<DateTime<Utc>>,
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

#[derive(serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSessionResponse {
    pub principal_subject: String,
    pub scopes: Vec<pmx_authz::Scope>,
    pub capabilities: Vec<pmx_authz::Operation>,
    pub no_remote_side_effect: bool,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CancelOrderRequest {
    pub account_id: String,
    pub order_id: String,
    pub execution_id: Option<String>,
    pub reason: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileOrderLocalRequest {
    pub account_id: String,
    pub order_id: String,
    pub remote_observation: RemoteOrderObservation,
    pub reason: String,
}

#[derive(serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileOrderLocalResponse {
    pub order_id: String,
    pub divergence: OrderLifecycleDivergence,
    pub updated_order: Option<OrderLifecycleRecord>,
    pub no_remote_side_effect: bool,
}
