use super::*;

pub(super) async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    require(&headers, Operation::ReadReport)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "NOT_READY",
            "executor_version": env!("CARGO_PKG_VERSION"),
            "contract_version": CONTRACT_VERSION,
            "checks": {
                "live_gateway": "not_configured",
                "database": state.service.storage_mode(),
                "signer": "not_configured",
                "auth": "enabled_distinct_tokens",
                "service_layer": "pmx_service_server_authoritative_id_bound_admin_audit"
            }
        })),
    ))
}
