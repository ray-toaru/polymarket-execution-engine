use super::*;
use crate::support::{
    api_error_with_correlation, correlation_id_from_headers, record_admin_audit,
    request_fingerprint, require,
};

pub async fn require_reconcile_context<T: serde::Serialize>(
    _state: &AppState,
    headers: &HeaderMap,
    operation: pmx_authz::Operation,
    req: &T,
) -> Result<(pmx_authz::Principal, String, Option<String>), ReconcileApiError> {
    let principal = require(headers, operation)?;
    let correlation_id = correlation_id_from_headers(headers);
    let fingerprint = request_fingerprint(req);
    Ok((principal, correlation_id, fingerprint))
}

pub async fn reject_bad_request(
    state: &AppState,
    principal: &pmx_authz::Principal,
    operation_name: &'static str,
    fingerprint: Option<String>,
    correlation_id: String,
    message: &str,
) -> Result<ReconcileApiError, ReconcileApiError> {
    record_admin_audit(
        state,
        principal,
        operation_name,
        fingerprint,
        Some(correlation_id.clone()),
        "REJECTED bad_request",
    )
    .await?;
    Ok(api_error_with_correlation(
        StatusCode::BAD_REQUEST,
        message,
        correlation_id,
    ))
}
