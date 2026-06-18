use pmx_core::{AccountId, RemoteOrderId, RemoteOrderObservation, TokenId};
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
pub struct RemoteOrder {
    pub remote_order_id: RemoteOrderId,
    pub account_id: AccountId,
    pub state: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveReadOperation {
    GetOrder,
    ListOpenOrders,
    ListFills,
    ListPositions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveReadOutcome {
    Observed,
    Missing,
    Blocked,
    RemoteRejected,
    RemoteUnknown,
    AuthenticationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveReadErrorCategory {
    RemoteRejected,
    RemoteUnknown,
    AuthenticationFailed,
    Disabled,
    SigningUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveReadNormalizedEvent {
    pub account_id: AccountId,
    pub operation: LiveReadOperation,
    pub outcome: LiveReadOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_order_id: Option<RemoteOrderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_category: Option<LiveReadErrorCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_error_summary: Option<String>,
    pub no_trading_side_effect: bool,
    pub redacted_fields: Vec<String>,
}

impl LiveReadNormalizedEvent {
    pub fn observed_order(operation: LiveReadOperation, remote_order: RemoteOrder) -> Self {
        Self {
            account_id: remote_order.account_id,
            operation,
            outcome: LiveReadOutcome::Observed,
            remote_order_id: Some(remote_order.remote_order_id),
            remote_state: Some(remote_order.state),
            error_category: None,
            redacted_error_summary: None,
            no_trading_side_effect: true,
            redacted_fields: live_read_redacted_fields(),
        }
    }

    pub fn from_gateway_error(
        account_id: AccountId,
        operation: LiveReadOperation,
        remote_order_id: Option<RemoteOrderId>,
        error: GatewayError,
    ) -> Self {
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
        Self {
            account_id,
            operation,
            outcome,
            remote_order_id,
            remote_state: None,
            error_category: Some(error_category),
            redacted_error_summary: summary,
            no_trading_side_effect: true,
            redacted_fields: live_read_redacted_fields(),
        }
    }
}

fn live_read_redacted_fields() -> Vec<String> {
    [
        "raw_remote_payload",
        "raw_error",
        "signed_payload",
        "signature",
        "api_key",
        "api_secret",
        "api_passphrase",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
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
