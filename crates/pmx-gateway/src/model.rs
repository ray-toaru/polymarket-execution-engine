use pmx_core::{
    AccountId, LiveReadErrorCategory, LiveReadNormalizedEvent, LiveReadOperation, LiveReadOutcome,
    RemoteOrderId, RemoteOrderObservation, TokenId,
};
use serde::{Deserialize, Serialize};

use crate::GatewayError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanOrder {
    pub execution_id: String,
    pub account_id: AccountId,
    pub token_id: TokenId,
    pub side: String,
    pub limit_price: String,
    pub size: String,
    pub time_in_force: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostOrderAck {
    pub remote_order_id: RemoteOrderId,
    pub accepted_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReconcileReadRequest {
    pub account_id: AccountId,
    pub remote_order_ids: Vec<RemoteOrderId>,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReconcileObservation {
    pub remote_order_id: RemoteOrderId,
    pub observation: RemoteOrderObservation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteReconcileReadReport {
    pub observations: Vec<RemoteReconcileObservation>,
    pub no_trading_side_effect: bool,
}

pub fn live_read_event_from_gateway_error(
    account_id: AccountId,
    operation: LiveReadOperation,
    remote_order_id: Option<RemoteOrderId>,
    error: GatewayError,
) -> LiveReadNormalizedEvent {
    let (outcome, error_category, summary) = match error {
        GatewayError::RemoteRejected(message) => (
            LiveReadOutcome::RemoteRejected,
            LiveReadErrorCategory::RemoteRejected,
            Some(redact_live_read_error_summary(&message)),
        ),
        GatewayError::RemoteUnknown(message) => (
            LiveReadOutcome::RemoteUnknown,
            LiveReadErrorCategory::RemoteUnknown,
            Some(redact_live_read_error_summary(&message)),
        ),
        GatewayError::AuthenticationFailed => (
            LiveReadOutcome::AuthenticationFailed,
            LiveReadErrorCategory::AuthenticationFailed,
            None,
        ),
        GatewayError::SigningUnavailable => (
            LiveReadOutcome::Blocked,
            LiveReadErrorCategory::SigningUnavailable,
            None,
        ),
        GatewayError::Disabled => (
            LiveReadOutcome::Blocked,
            LiveReadErrorCategory::Disabled,
            None,
        ),
    };
    LiveReadNormalizedEvent::error(
        account_id,
        operation,
        outcome,
        remote_order_id,
        error_category,
        summary,
    )
}

fn redact_assignment_value(input: &str, key: &str) -> String {
    let marker = format!("{key}=");
    let marker_lower = marker.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.to_ascii_lowercase().find(&marker_lower) {
        out.push_str(&rest[..idx]);
        out.push_str(&rest[idx..idx + marker.len()]);
        out.push_str("[REDACTED]");
        let after = &rest[idx + marker.len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '&'))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn looks_like_hex_private_key(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '(' | '"' | '\''));
    let Some(hex) = trimmed.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn redact_live_read_error_summary(input: &str) -> String {
    let mut out = input.to_owned();
    for key in [
        "POLY_PRIVATE_KEY",
        "POLY_API_KEY",
        "POLY_API_SECRET",
        "POLY_API_PASSPHRASE",
        "PRIVATE_KEY",
        "API_KEY",
        "API_SECRET",
        "API_PASSPHRASE",
        "SIGNATURE",
        "SIGNED_PAYLOAD",
    ] {
        out = redact_assignment_value(&out, key);
    }
    out.split_whitespace()
        .map(|token| {
            if looks_like_hex_private_key(token) {
                "0x[REDACTED]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(240)
        .collect()
}
