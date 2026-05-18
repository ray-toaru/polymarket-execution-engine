use crate::backend::AppState;
use crate::model::*;
use crate::support::{
    ApiResult, api_error_with_correlation, correlation_id_from_headers, record_admin_audit,
    request_fingerprint, require, service_error,
};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use pmx_authz::Operation;
use pmx_core::*;
use pmx_store::{AdminAuditEvent, AdminAuditQuery, ExecutionLifecycleEvent};
use uuid::Uuid;

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

pub(crate) async fn set_kill_switch(
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
