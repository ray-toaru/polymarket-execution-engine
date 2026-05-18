use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelRequestedNonLivePayload<'a> {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<&'a str>,
    pub reason_len: usize,
    pub no_remote_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileObservedNonLivePayload<'a> {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<&'a str>,
    pub reason_len: usize,
    pub no_remote_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLifecycleDivergenceNonLivePayload<'a> {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<&'a str>,
    pub operator_required: bool,
    pub reason_len: usize,
    pub classification: String,
    pub no_remote_side_effect: bool,
}

pub fn cancel_requested_non_live(
    correlation_id: Option<&str>,
    reason_len: usize,
) -> serde_json::Value {
    serde_json::to_value(CancelRequestedNonLivePayload {
        kind: "cancel_requested_non_live",
        correlation_id,
        reason_len,
        no_remote_side_effect: true,
    })
    .expect("cancel requested non-live payload is serializable")
}

pub fn reconcile_observed_non_live(
    correlation_id: Option<&str>,
    reason_len: usize,
) -> serde_json::Value {
    serde_json::to_value(ReconcileObservedNonLivePayload {
        kind: "reconcile_observed_non_live",
        correlation_id,
        reason_len,
        no_remote_side_effect: true,
    })
    .expect("reconcile observed non-live payload is serializable")
}

pub fn order_lifecycle_divergence_non_live(
    correlation_id: Option<&str>,
    operator_required: bool,
    reason_len: usize,
    classification: String,
) -> serde_json::Value {
    serde_json::to_value(OrderLifecycleDivergenceNonLivePayload {
        kind: "order_lifecycle_divergence_non_live",
        correlation_id,
        operator_required,
        reason_len,
        classification,
        no_remote_side_effect: true,
    })
    .expect("order lifecycle divergence non-live payload is serializable")
}
