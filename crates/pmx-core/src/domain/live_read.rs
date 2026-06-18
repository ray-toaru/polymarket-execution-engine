use serde::{Deserialize, Serialize};

use crate::{AccountId, RemoteOrderId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteOrder {
    pub remote_order_id: RemoteOrderId,
    pub account_id: AccountId,
    pub state: String,
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

    pub fn error(
        account_id: AccountId,
        operation: LiveReadOperation,
        outcome: LiveReadOutcome,
        remote_order_id: Option<RemoteOrderId>,
        error_category: LiveReadErrorCategory,
        redacted_error_summary: Option<String>,
    ) -> Self {
        Self {
            account_id,
            operation,
            outcome,
            remote_order_id,
            remote_state: None,
            error_category: Some(error_category),
            redacted_error_summary,
            no_trading_side_effect: true,
            redacted_fields: live_read_redacted_fields(),
        }
    }
}

pub fn live_read_redacted_fields() -> Vec<String> {
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
