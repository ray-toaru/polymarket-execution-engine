use axum::{Json, extract::State, http::HeaderMap};

use crate::backend::AppState;
use crate::model::{ReconcileOrderLocalRequest, ReconcileOrderLocalResponse};
use crate::support::{ApiResult, api_error_with_correlation, record_admin_audit, service_error};

use super::support::require_local_reconcile_request;

pub(crate) async fn reconcile_order_local(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReconcileOrderLocalRequest>,
) -> ApiResult<ReconcileOrderLocalResponse> {
    let (principal, correlation_id, fingerprint) =
        require_local_reconcile_request(&state, &headers, &req).await?;
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
            axum::http::StatusCode::NOT_FOUND,
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
        axum::http::StatusCode::ACCEPTED,
        Json(ReconcileOrderLocalResponse {
            order_id: req.order_id,
            divergence,
            updated_order,
            no_remote_side_effect: true,
        }),
    ))
}
