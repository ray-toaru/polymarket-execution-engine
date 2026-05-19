use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use pmx_core::ReconcileRequest;

use crate::backend::AppState;

#[path = "support/context.rs"]
mod context;

#[path = "support/local.rs"]
mod local;

#[path = "support/placeholder.rs"]
mod placeholder;

pub(super) use local::require_local_reconcile_request;
pub(super) use placeholder::require_reconcile_request;

pub(super) type ReconcileApiError = (StatusCode, Json<serde_json::Value>);

pub(super) type ReconcileRequestParts = (
    pmx_authz::Principal,
    String,
    Option<String>,
    Option<(String, pmx_core::RemoteOrderObservation)>,
);

pub(super) async fn require_reconcile_context<T: serde::Serialize>(
    state: &AppState,
    headers: &HeaderMap,
    operation: pmx_authz::Operation,
    req: &T,
) -> Result<(pmx_authz::Principal, String, Option<String>), ReconcileApiError> {
    context::require_reconcile_context(state, headers, operation, req).await
}

pub(super) async fn reject_bad_request(
    state: &AppState,
    principal: &pmx_authz::Principal,
    operation_name: &'static str,
    fingerprint: Option<String>,
    correlation_id: String,
    message: &str,
) -> Result<ReconcileApiError, ReconcileApiError> {
    context::reject_bad_request(
        state,
        principal,
        operation_name,
        fingerprint,
        correlation_id,
        message,
    )
    .await
}
