use crate::backend::AppState;
use axum::{
    Json,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};
use pmx_authz::{Operation, Principal, Scope, authorize};
use pmx_core::canonical_json_sha256;
use pmx_service::ServiceError;
use pmx_store::{AdminAuditEvent, StoreError};
use uuid::Uuid;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenConfig {
    pub service_token: String,
    pub admin_token: String,
}

pub fn validate_auth_config_from_env() -> Result<AuthTokenConfig, String> {
    let admin_token = std::env::var("PM_EXEC_ADMIN_TOKEN").unwrap_or_default();
    let service_token = std::env::var("PM_EXEC_SERVICE_TOKEN").unwrap_or_default();
    if service_token.is_empty() {
        return Err("PM_EXEC_SERVICE_TOKEN must be set".into());
    }
    if admin_token.is_empty() {
        return Err("PM_EXEC_ADMIN_TOKEN must be set".into());
    }
    if service_token == admin_token {
        return Err("PM_EXEC_SERVICE_TOKEN and PM_EXEC_ADMIN_TOKEN must be distinct".into());
    }
    Ok(AuthTokenConfig {
        service_token,
        admin_token,
    })
}

pub(crate) fn principal_from_headers(
    headers: &HeaderMap,
) -> Result<Principal, (StatusCode, Json<serde_json::Value>)> {
    let auth_config = validate_auth_config_from_env().map_err(|err| {
        api_error_with_correlation(
            StatusCode::INTERNAL_SERVER_ERROR,
            err,
            correlation_id_from_headers(headers),
        )
    })?;
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            api_error_with_correlation(
                StatusCode::UNAUTHORIZED,
                "missing Authorization bearer token",
                correlation_id_from_headers(headers),
            )
        })?;
    let Some(token) = header.strip_prefix("Bearer ") else {
        return Err(api_error_with_correlation(
            StatusCode::UNAUTHORIZED,
            "Authorization must use Bearer token",
            correlation_id_from_headers(headers),
        ));
    };

    if token == auth_config.admin_token {
        return Ok(Principal {
            subject: "admin-token".into(),
            scopes: vec![Scope::Admin],
        });
    }
    if token == auth_config.service_token {
        return Ok(Principal {
            subject: "service-token".into(),
            scopes: vec![Scope::Service],
        });
    }
    Err(api_error_with_correlation(
        StatusCode::FORBIDDEN,
        "token is not authorized",
        correlation_id_from_headers(headers),
    ))
}

pub(crate) fn require(
    headers: &HeaderMap,
    op: Operation,
) -> Result<Principal, (StatusCode, Json<serde_json::Value>)> {
    let principal = principal_from_headers(headers)?;
    authorize(&principal, op).map_err(|err| {
        api_error_with_correlation(
            StatusCode::FORBIDDEN,
            err.to_string(),
            correlation_id_from_headers(headers),
        )
    })?;
    Ok(principal)
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

pub(crate) async fn record_admin_audit(
    state: &AppState,
    principal: &Principal,
    operation: &'static str,
    request_fingerprint: Option<String>,
    correlation_id: Option<String>,
    result: impl Into<String>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    state
        .service
        .record_admin_audit_event(AdminAuditEvent {
            audit_id: None,
            principal_subject: principal.subject.clone(),
            operation: operation.into(),
            request_fingerprint,
            correlation_id,
            result: result.into(),
            created_at: None,
        })
        .await
        .map_err(service_error)
}
