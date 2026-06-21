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
    pub admin_read_token: Option<String>,
    pub admin_cancel_token: Option<String>,
    pub emergency_operator_token: Option<String>,
}

pub fn validate_auth_config_from_env() -> Result<AuthTokenConfig, String> {
    let config = AuthTokenConfig {
        service_token: required_env_token("PMX_API_SERVICE_TOKEN"),
        admin_token: required_env_token("PMX_API_ADMIN_TOKEN"),
        admin_read_token: optional_env_token("PMX_API_ADMIN_READ_TOKEN"),
        admin_cancel_token: optional_env_token("PMX_API_ADMIN_CANCEL_TOKEN"),
        emergency_operator_token: optional_env_token("PMX_API_EMERGENCY_OPERATOR_TOKEN"),
    };
    validate_auth_config(config)
}

fn required_env_token(key: &str) -> String {
    std::env::var(key).unwrap_or_default().trim().to_owned()
}

fn optional_env_token(key: &str) -> Option<String> {
    let value = required_env_token(key);
    if value.is_empty() { None } else { Some(value) }
}

fn validate_auth_config(config: AuthTokenConfig) -> Result<AuthTokenConfig, String> {
    let service_token = config.service_token.as_str();
    let admin_token = config.admin_token.as_str();
    if service_token.is_empty() {
        return Err("PMX_API_SERVICE_TOKEN must be set".into());
    }
    if admin_token.is_empty() {
        return Err("PMX_API_ADMIN_TOKEN must be set".into());
    }
    let named_tokens = [
        ("PMX_API_SERVICE_TOKEN", Some(service_token)),
        ("PMX_API_ADMIN_TOKEN", Some(admin_token)),
        (
            "PMX_API_ADMIN_READ_TOKEN",
            config.admin_read_token.as_deref(),
        ),
        (
            "PMX_API_ADMIN_CANCEL_TOKEN",
            config.admin_cancel_token.as_deref(),
        ),
        (
            "PMX_API_EMERGENCY_OPERATOR_TOKEN",
            config.emergency_operator_token.as_deref(),
        ),
    ];
    for (idx, (left_name, left_token)) in named_tokens.iter().enumerate() {
        let Some(left_token) = left_token else {
            continue;
        };
        if left_token.is_empty() {
            return Err(format!("{left_name} must not be empty when set"));
        }
        for (right_name, right_token) in named_tokens.iter().skip(idx + 1) {
            let Some(right_token) = right_token else {
                continue;
            };
            if left_token == right_token {
                return Err(format!("{left_name} and {right_name} must be distinct"));
            }
        }
    }
    Ok(config)
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

    if let Some(principal) = principal_from_bearer_token(token, &auth_config) {
        return Ok(principal);
    }
    Err(api_error_with_correlation(
        StatusCode::FORBIDDEN,
        "token is not authorized",
        correlation_id_from_headers(headers),
    ))
}

fn principal_from_bearer_token(token: &str, auth_config: &AuthTokenConfig) -> Option<Principal> {
    let candidates = [
        (
            auth_config.admin_token.as_str(),
            "admin-token",
            Scope::Admin,
        ),
        (
            auth_config.service_token.as_str(),
            "service-token",
            Scope::Service,
        ),
        (
            auth_config.admin_read_token.as_deref().unwrap_or_default(),
            "admin-read-token",
            Scope::AdminRead,
        ),
        (
            auth_config
                .admin_cancel_token
                .as_deref()
                .unwrap_or_default(),
            "admin-cancel-token",
            Scope::AdminCancel,
        ),
        (
            auth_config
                .emergency_operator_token
                .as_deref()
                .unwrap_or_default(),
            "emergency-operator-token",
            Scope::EmergencyOperator,
        ),
    ];
    for (candidate, subject, scope) in candidates {
        if !candidate.is_empty() && constant_time_eq(token.as_bytes(), candidate.as_bytes()) {
            return Some(Principal {
                subject: subject.into(),
                scopes: vec![scope],
            });
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    fn split_config() -> AuthTokenConfig {
        AuthTokenConfig {
            service_token: "svc".into(),
            admin_token: "admin".into(),
            admin_read_token: Some("admin-read".into()),
            admin_cancel_token: Some("admin-cancel".into()),
            emergency_operator_token: Some("emergency".into()),
        }
    }

    #[test]
    fn split_admin_tokens_resolve_to_limited_scopes() {
        let cfg = split_config();
        assert_eq!(
            principal_from_bearer_token("admin-read", &cfg)
                .unwrap()
                .scopes,
            vec![Scope::AdminRead]
        );
        assert_eq!(
            principal_from_bearer_token("admin-cancel", &cfg)
                .unwrap()
                .scopes,
            vec![Scope::AdminCancel]
        );
        assert_eq!(
            principal_from_bearer_token("emergency", &cfg)
                .unwrap()
                .scopes,
            vec![Scope::EmergencyOperator]
        );
        assert_eq!(
            principal_from_bearer_token("admin", &cfg).unwrap().scopes,
            vec![Scope::Admin]
        );
    }

    #[test]
    fn duplicate_split_admin_tokens_fail_closed() {
        let mut cfg = split_config();
        cfg.admin_cancel_token = Some("admin-read".into());
        let err = validate_auth_config(cfg).expect_err("duplicate tokens must fail closed");
        assert!(err.contains("distinct"));
    }
}
