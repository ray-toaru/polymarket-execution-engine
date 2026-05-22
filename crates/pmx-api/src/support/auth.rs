use axum::{
    Json,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};
use pmx_authz::{Operation, Principal, Scope, authorize};

use crate::support::{api_error_with_correlation, correlation_id_from_headers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenConfig {
    pub service_token: String,
    pub admin_token: String,
}

pub fn validate_auth_config_from_env() -> Result<AuthTokenConfig, String> {
    let admin_token = std::env::var("PM_EXEC_ADMIN_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let service_token = std::env::var("PM_EXEC_SERVICE_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_owned();
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

    if constant_time_eq(token.as_bytes(), auth_config.admin_token.as_bytes()) {
        return Ok(Principal {
            subject: "admin-token".into(),
            scopes: vec![Scope::Admin],
        });
    }
    if constant_time_eq(token.as_bytes(), auth_config.service_token.as_bytes()) {
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

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = if left.len() == right.len() { 0 } else { 1 };
    let max_len = left.len().max(right.len());
    for idx in 0..max_len {
        let left_byte = left.get(idx).copied().unwrap_or(0);
        let right_byte = right.get(idx).copied().unwrap_or(0);
        diff |= left_byte ^ right_byte;
    }
    diff == 0
}
