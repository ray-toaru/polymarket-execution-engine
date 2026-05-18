use pmx_core::{AccountId, RemoteOrderId, RemoteOrderObservation, TokenId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanOrder {
    pub execution_id: String,
    pub account_id: AccountId,
    pub token_id: TokenId,
    pub limit_price: String,
    pub size: String,
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
