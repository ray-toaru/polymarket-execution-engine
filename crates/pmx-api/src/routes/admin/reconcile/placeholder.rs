use axum::{Json, extract::State, http::HeaderMap};
use pmx_core::{ReconcileReport, ReconcileRequest, redacted_payload_envelope};
use pmx_store::ExecutionLifecycleEvent;
use uuid::Uuid;

use crate::backend::AppState;
use crate::support::{ApiResult, api_error_with_correlation, record_admin_audit, service_error};

use super::support::require_reconcile_request;

pub(crate) async fn reconcile_placeholder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReconcileRequest>,
) -> ApiResult<ReconcileReport> {
    let (principal, correlation_id, fingerprint, local_reconcile) =
        require_reconcile_request(&state, &headers, &req).await?;
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
                axum::http::StatusCode::NOT_FOUND,
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
    Ok((axum::http::StatusCode::ACCEPTED, Json(report)))
}
