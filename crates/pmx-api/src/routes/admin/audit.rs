use crate::backend::AppState;
use crate::model::AuditQuery;
use crate::support::{ApiResult, require, service_error};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;
use pmx_store::{AdminAuditEvent, AdminAuditQuery};

pub(crate) async fn list_admin_audit_events(
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
