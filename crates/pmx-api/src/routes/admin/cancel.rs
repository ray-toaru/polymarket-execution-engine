use crate::backend::AppState;
use crate::model::CancelOrderRequest;
use crate::support::{
    ApiResult, api_error_with_correlation, correlation_id_from_headers, record_admin_audit,
    request_fingerprint, require, service_error,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;
use pmx_core::{CancelReceipt, CancelState, redacted_payload_envelope};
use pmx_store::ExecutionLifecycleEvent;
use uuid::Uuid;

pub(crate) async fn cancel_order_placeholder(
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
