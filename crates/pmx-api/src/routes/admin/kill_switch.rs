use crate::backend::AppState;
use crate::support::{
    ApiResult, correlation_id_from_headers, record_admin_audit, request_fingerprint, require,
    service_error,
};
use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use chrono::Utc;
use pmx_authz::Operation;
use pmx_core::{KillSwitchReceipt, KillSwitchRequest, KillSwitchScope};

pub(crate) async fn set_kill_switch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KillSwitchRequest>,
) -> ApiResult<KillSwitchReceipt> {
    let principal = require(&headers, Operation::KillSwitch)?;
    let correlation_id = correlation_id_from_headers(&headers);
    let fingerprint = request_fingerprint(&req);
    let state_change = match req.scope {
        KillSwitchScope::Account => {
            let Some(account_id) = req.account_id.as_ref() else {
                return Err(crate::support::api_error_with_correlation(
                    StatusCode::BAD_REQUEST,
                    "account_id is required for ACCOUNT kill-switch scope",
                    correlation_id.clone(),
                ));
            };
            if account_id.0.trim().is_empty() {
                return Err(crate::support::api_error_with_correlation(
                    StatusCode::BAD_REQUEST,
                    "account_id is required for ACCOUNT kill-switch scope",
                    correlation_id.clone(),
                ));
            }
            state
                .service
                .set_account_kill_switch(account_id, req.enabled, &req.reason)
                .await
                .map_err(service_error)?
        }
        KillSwitchScope::Global => {
            if req.account_id.is_some() {
                return Err(crate::support::api_error_with_correlation(
                    StatusCode::BAD_REQUEST,
                    "account_id must be omitted for GLOBAL kill-switch scope",
                    correlation_id.clone(),
                ));
            }
            state
                .service
                .set_global_kill_switch(req.enabled, &req.reason)
                .await
                .map_err(service_error)?
        }
    };
    let receipt = KillSwitchReceipt {
        scope: state_change.scope,
        account_id: state_change.account_id,
        enabled: state_change.enabled,
        changed_at: Utc::now(),
        effective_at: state_change.effective_at,
        state_version: state_change.state_version,
        persisted: true,
        reason: req.reason,
    };
    record_admin_audit(
        &state,
        &principal,
        "KillSwitch",
        fingerprint,
        Some(correlation_id.clone()),
        format!(
            "ACCEPTED scope={:?} enabled={} account_id={} state_version={} correlation_id={}",
            receipt.scope,
            receipt.enabled,
            receipt
                .account_id
                .as_ref()
                .map(|account_id| account_id.0.as_str())
                .unwrap_or("<global>"),
            receipt.state_version,
            correlation_id
        ),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}
