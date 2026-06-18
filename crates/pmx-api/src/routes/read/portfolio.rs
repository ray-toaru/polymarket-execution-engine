use super::*;
use pmx_core::{AccountId, PortfolioProjection, RiskDecision, RiskLimits};

pub(crate) async fn record_portfolio_projection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(projection): Json<PortfolioProjection>,
) -> ApiResult<PortfolioProjectionRecordResponse> {
    require(&headers, Operation::CaptureSnapshot)?;
    let account_id = projection.account_id.0.clone();
    let observed_at_ms = projection.observed_at_ms;
    state
        .service
        .record_portfolio_projection(projection)
        .await
        .map_err(service_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(PortfolioProjectionRecordResponse {
            account_id,
            observed_at_ms,
            no_remote_side_effect: true,
        }),
    ))
}

pub(crate) async fn get_portfolio_projection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
) -> ApiResult<PortfolioProjection> {
    require(&headers, Operation::ReadReport)?;
    let projection = state
        .service
        .load_portfolio_projection(&AccountId(account_id))
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(projection)))
}

pub(crate) async fn assess_portfolio_risk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
    Json(limits): Json<RiskLimits>,
) -> ApiResult<RiskDecision> {
    require(&headers, Operation::ReadReport)?;
    let decision = state
        .service
        .assess_portfolio_risk(&AccountId(account_id), limits)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(decision)))
}
