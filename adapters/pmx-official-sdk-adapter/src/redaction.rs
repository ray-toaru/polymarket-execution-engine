use crate::{
    L2_API_KEY_VAR, L2_API_PASSPHRASE_VAR, L2_API_SECRET_VAR, OfficialSdkErrorCategory,
    OfficialSdkNormalizedError, PRIVATE_KEY_VAR_NAME, REDACTED,
};
use pmx_gateway::GatewayError;

pub fn gateway_error_from_normalized_sdk_error(
    normalized: &OfficialSdkNormalizedError,
) -> GatewayError {
    match normalized.category {
        OfficialSdkErrorCategory::AuthenticationFailed => GatewayError::AuthenticationFailed,
        OfficialSdkErrorCategory::ValidationFailed | OfficialSdkErrorCategory::RemoteRejected => {
            GatewayError::RemoteRejected(redact_sensitive_text(&normalized.message))
        }
        OfficialSdkErrorCategory::RemoteUnknown
        | OfficialSdkErrorCategory::WebSocketFailed
        | OfficialSdkErrorCategory::Geoblocked
        | OfficialSdkErrorCategory::Internal => {
            GatewayError::RemoteUnknown(redact_sensitive_text(&normalized.message))
        }
    }
}

fn redact_assignment_value(input: &str, key: &str) -> String {
    let marker = format!("{key}=");
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(&marker) {
        out.push_str(&rest[..idx]);
        out.push_str(&marker);
        out.push_str(REDACTED);
        let after = &rest[idx + marker.len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '&'))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn redact_known_env_values(input: &str) -> String {
    let mut out = input.to_owned();
    for key in [
        PRIVATE_KEY_VAR_NAME,
        L2_API_KEY_VAR,
        L2_API_SECRET_VAR,
        L2_API_PASSPHRASE_VAR,
    ] {
        if let Ok(value) = std::env::var(key)
            && value.len() >= 4
        {
            out = out.replace(&value, REDACTED);
        }
        out = redact_assignment_value(&out, key);
    }
    out
}

fn looks_like_hex_private_key(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '(' | '"' | '\''));
    let Some(hex) = trimmed.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn redact_sensitive_text(input: &str) -> String {
    let env_redacted = redact_known_env_values(input);
    env_redacted
        .split_whitespace()
        .map(|token| {
            if looks_like_hex_private_key(token) {
                "0x[REDACTED]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn redact_normalized_error(error: &OfficialSdkNormalizedError) -> OfficialSdkNormalizedError {
    let mut redacted = error.clone();
    redacted.message = redact_sensitive_text(&redacted.message);
    redacted
}
