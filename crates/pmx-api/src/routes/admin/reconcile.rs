use crate::backend::AppState;
use crate::model::{ReconcileOrderLocalRequest, ReconcileOrderLocalResponse};
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
use pmx_core::{ReconcileReport, ReconcileRequest, redacted_payload_envelope};
use pmx_store::ExecutionLifecycleEvent;
use uuid::Uuid;

pub(crate) async fn reconcile_placeholder(
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

pub(crate) async fn reconcile_order_local(
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
