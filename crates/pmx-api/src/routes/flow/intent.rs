use super::*;

pub(crate) async fn normalize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(intent): Json<TradeIntent>,
) -> ApiResult<NormalizedIntent> {
    require(&headers, Operation::NormalizeIntent)?;
    let normalized = state
        .service
        .normalize(intent)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(normalized)))
}

pub(crate) async fn capture_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(intent): Json<NormalizedIntent>,
) -> ApiResult<FeasibilitySnapshot> {
    require(&headers, Operation::CaptureSnapshot)?;
    let snapshot = state
        .service
        .capture_snapshot(intent)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(snapshot)))
}

pub(crate) async fn decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DecisionRequest>,
) -> ApiResult<ConstraintDecision> {
    require(&headers, Operation::EvaluateDecision)?;
    let decision = state
        .service
        .evaluate_decision_by_id(pmx_service::DecisionByIdRequest {
            normalized_intent_id: req.normalized_intent_id,
            snapshot_id: req.snapshot_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(decision)))
}
