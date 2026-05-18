use axum::{Json, http::StatusCode};
use pmx_core::canonical_json_sha256;
use pmx_service::ServiceError;
use pmx_store::StoreError;
use uuid::Uuid;

use axum::http::HeaderMap;

pub(crate) type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<serde_json::Value>)>;

fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

pub(crate) fn api_error_with_correlation(
    status: StatusCode,
    message: impl Into<String>,
    correlation_id: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": message.into(),
            "correlation_id": correlation_id.into(),
        })),
    )
}

pub(crate) fn service_error(err: ServiceError) -> (StatusCode, Json<serde_json::Value>) {
    match err {
        ServiceError::BadRequest(msg) => api_error(StatusCode::BAD_REQUEST, msg),
        ServiceError::Conflict(msg) => api_error(StatusCode::CONFLICT, msg),
        ServiceError::InProgress { retry_after_ms } => api_error(
            StatusCode::CONFLICT,
            format!("submit attempt already in progress; retry_after_ms={retry_after_ms}"),
        ),
        ServiceError::Store(StoreError::NotFound(msg)) => api_error(StatusCode::NOT_FOUND, msg),
        ServiceError::Store(StoreError::Conflict(msg)) => api_error(StatusCode::CONFLICT, msg),
        ServiceError::Store(other) => {
            api_error(StatusCode::INTERNAL_SERVER_ERROR, other.to_string())
        }
        ServiceError::Internal(msg) => api_error(StatusCode::INTERNAL_SERVER_ERROR, msg),
    }
}

pub(crate) fn correlation_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

pub(crate) fn request_fingerprint<T: serde::Serialize>(request: &T) -> Option<String> {
    canonical_json_sha256(request).ok().map(|hash| hash.0)
}
