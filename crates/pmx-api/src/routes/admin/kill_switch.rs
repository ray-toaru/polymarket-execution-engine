use crate::backend::AppState;
use crate::support::{
    ApiResult, correlation_id_from_headers, record_admin_audit, request_fingerprint, require,
};
use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use chrono::Utc;
use pmx_authz::Operation;
use pmx_core::{KillSwitchReceipt, KillSwitchRequest};

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
