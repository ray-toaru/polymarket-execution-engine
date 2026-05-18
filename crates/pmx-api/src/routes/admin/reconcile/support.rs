use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;
use pmx_core::ReconcileRequest;

use crate::backend::AppState;
use crate::support::{
    api_error_with_correlation, correlation_id_from_headers, record_admin_audit,
    request_fingerprint, require,
};

pub(super) async fn require_reconcile_request(
    state: &AppState,
    headers: &HeaderMap,
    req: &ReconcileRequest,
) -> Result<
    (
        pmx_authz::Principal,
        String,
        Option<String>,
        Option<(String, pmx_core::RemoteOrderObservation)>,
    ),
    (StatusCode, Json<serde_json::Value>),
> {
    let principal = require(headers, Operation::Reconcile)?;
    let correlation_id = correlation_id_from_headers(headers);
    let fingerprint = request_fingerprint(req);
    if req.reason.trim().is_empty() {
        record_admin_audit(
            state,
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
                state,
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
    Ok((principal, correlation_id, fingerprint, local_reconcile))
}

pub(super) async fn require_local_reconcile_request(
    state: &AppState,
    headers: &HeaderMap,
    req: &crate::model::ReconcileOrderLocalRequest,
) -> Result<(pmx_authz::Principal, String, Option<String>), (StatusCode, Json<serde_json::Value>)> {
    let principal = require(headers, Operation::Reconcile)?;
    let correlation_id = correlation_id_from_headers(headers);
    let fingerprint = request_fingerprint(req);
    if req.account_id.trim().is_empty()
        || req.order_id.trim().is_empty()
        || req.reason.trim().is_empty()
    {
        record_admin_audit(
            state,
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
    Ok((principal, correlation_id, fingerprint))
}
