use super::*;

pub(crate) async fn list_sign_only_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<SignOnlyLifecycleRecord>> {
    require(&headers, Operation::ReadReport)?;
    let records = state
        .service
        .list_sign_only_lifecycle_events(SignOnlyLifecycleQuery {
            execution_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(records)))
}

pub(crate) async fn list_execution_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<ExecutionLifecycleEvent>> {
    require(&headers, Operation::ReadReport)?;
    let events = state
        .service
        .list_execution_lifecycle_events(ExecutionLifecycleQuery {
            execution_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(events)))
}

pub(crate) async fn list_order_lifecycle_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    Query(query): Query<EventListQuery>,
) -> ApiResult<Vec<OrderLifecycleEventRecord>> {
    require(&headers, Operation::ReadReport)?;
    let events = state
        .service
        .list_order_lifecycle_events(OrderLifecycleEventQuery {
            order_id,
            limit: query.limit.unwrap_or(100),
            before_event_id: query.before_event_id,
        })
        .await
        .map_err(service_error)?;
    Ok((StatusCode::OK, Json(events)))
}
