use super::*;

pub(crate) async fn get_submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
) -> ApiResult<SubmitReceipt> {
    require(&headers, Operation::ReadReport)?;
    let receipt = state
        .service
        .load_submit_receipt(&execution_id)
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(receipt)))
}
