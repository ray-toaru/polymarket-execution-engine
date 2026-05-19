use super::*;

pub(crate) async fn list_runtime_worker_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RuntimeWorkerStatusListQuery>,
) -> ApiResult<RuntimeWorkerStatusReport> {
    require(&headers, Operation::ReadReport)?;
    let report = state
        .service
        .list_runtime_worker_status(RuntimeWorkerStatusQuery {
            account_id: query.account_id,
            limit: query.limit.unwrap_or(100),
            before_observed_at: query.before_observed_at,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(report)))
}
