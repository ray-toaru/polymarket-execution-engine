use super::*;

pub(crate) async fn record_sign_only_lifecycle_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(record): Json<SignOnlyLifecycleRecord>,
) -> ApiResult<SignOnlyLifecycleRecord> {
    require(&headers, Operation::RecordSignOnlyLifecycle)?;
    let recorded = state
        .service
        .record_sign_only_lifecycle_event(record)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::ACCEPTED, Json(recorded)))
}

pub(crate) async fn record_standard_sign_only_construction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<StandardSignOnlyConstructionRequest>,
) -> ApiResult<StandardSignOnlyConstructionReceipt> {
    require(&headers, Operation::RecordSignOnlyLifecycle)?;
    let receipt = state
        .service
        .record_standard_sign_only_construction(req)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}
