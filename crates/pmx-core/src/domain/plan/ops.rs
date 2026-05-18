use serde::{Deserialize, Serialize};

use crate::{AccountId, ExecutionId, InternalOrderId, QuantityBound, RemoteOrderObservation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CancelState {
    Requested,
    RemoteAccepted,
    ConfirmedCanceled,
    NotCanceled,
    RemoteUnknown,
    ReconcileRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelReceipt {
    pub cancel_id: String,
    pub order_id: String,
    pub state: CancelState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReservationState {
    Pending,
    Active,
    Released,
    Consumed,
    Orphaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderReservation {
    pub reservation_id: String,
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub internal_order_id: Option<InternalOrderId>,
    pub quantity_bound: QuantityBound,
    pub state: ReservationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillSwitchRequest {
    pub enabled: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillSwitchReceipt {
    pub enabled: bool,
    pub changed_at: chrono::DateTime<chrono::Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub account_id: AccountId,
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_observation: Option<RemoteOrderObservation>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileReport {
    pub reconcile_id: String,
    pub status: String,
    pub checked_orders: u64,
    pub findings: Vec<String>,
}
