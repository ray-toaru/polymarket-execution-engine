use axum::{Json, http::StatusCode};
use pmx_authz::Principal;
use pmx_store::AdminAuditEvent;

use crate::backend::AppState;
use crate::support::service_error;

pub(crate) async fn record_admin_audit(
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
