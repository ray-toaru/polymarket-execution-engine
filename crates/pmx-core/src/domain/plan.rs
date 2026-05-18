use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AccountId, DecimalString, ExecutionId, HashValue, InternalOrderId, QuantityBound};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionStatus {
    Allow,
    Block,
    CloseOnly,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockReason {
    KillSwitchOn,
    GeoblockBlocked,
    GeoblockUnknown,
    GeoblockError,
    WorkerDegraded,
    WorkerStale,
    WorkerUnknown,
    CollateralProfileMissing,
    CollateralProfileUnknown,
    UnsupportedQuantityBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintDecision {
    pub decision_id: String,
    pub decision_hash: HashValue,
    pub status: DecisionStatus,
    pub reasons: Vec<BlockReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlanSummary {
    pub execution_id: String,
    pub account_id: AccountId,
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub plan_hash: HashValue,
    pub status: PlanStatus,
    pub max_exposure: DecimalString,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanStatus {
    Ready,
    Blocked,
}

// Internal-only type. Do not expose in OpenAPI or public control-plane clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOrderEnvelope {
    pub internal_order_id: InternalOrderId,
    pub account_id: AccountId,
    pub signer_fingerprint: String,
    pub signed_payload_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedPayloadEnvelope {
    pub schema_version: u32,
    pub kind: String,
    pub correlation_id: Option<String>,
    pub redacted_fields: Vec<String>,
    pub body: Value,
}

pub fn redacted_payload_envelope(
    kind: impl Into<String>,
    correlation_id: Option<String>,
    body: Value,
) -> Value {
    let envelope = RedactedPayloadEnvelope {
        schema_version: 1,
        kind: kind.into(),
        correlation_id,
        redacted_fields: vec![
            "private_key".into(),
            "clob_secret".into(),
            "signed_payload".into(),
            "signed_order_envelope".into(),
        ],
        body,
    };
    serde_json::to_value(envelope).expect("redacted payload envelope serializes")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitStatus {
    Accepted,
    Posted,
    PartialRemoteUnknown,
    RemoteUnknown,
    Rejected,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitReceipt {
    pub execution_id: String,
    pub receipt_id: String,
    pub status: SubmitStatus,
    pub executor_version: String,
    pub contract_version: String,
}

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
    pub remote_observation: Option<crate::RemoteOrderObservation>,
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
