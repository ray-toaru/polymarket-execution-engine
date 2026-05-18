use crate::backend::AppState;
use crate::model::*;
use crate::support::{ApiResult, require, service_error};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use pmx_authz::Operation;
use pmx_core::{SignOnlyLifecycleRecord, SubmitReceipt};
use pmx_store::{
    ExecutionLifecycleEvent, ExecutionLifecycleQuery, OrderLifecycleEventQuery,
    OrderLifecycleEventRecord, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
    SignOnlyLifecycleQuery,
};

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
