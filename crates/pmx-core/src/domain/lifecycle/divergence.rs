use serde::{Deserialize, Serialize};

use super::{OrderEventKind, OrderLifecycleState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconcileAction {
    Noop,
    QueryRemoteOpenOrder,
    ConfirmMissingOrEscalate,
    OperatorRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemoteOrderObservation {
    Open,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderLifecycleDivergenceKind {
    None,
    LocalRemoteUnknownRemoteOpen,
    LocalRemoteUnknownRemoteMissing,
    LocalRemoteUnknownStillUnknown,
    TerminalLocalRemoteMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLifecycleDivergence {
    pub kind: OrderLifecycleDivergenceKind,
    pub event: Option<OrderEventKind>,
    pub operator_required: bool,
    pub no_remote_side_effect: bool,
    pub reason: String,
}

pub fn reconcile_action_for_lifecycle(state: &OrderLifecycleState) -> ReconcileAction {
    match state {
        OrderLifecycleState::RemoteUnknown => ReconcileAction::QueryRemoteOpenOrder,
        OrderLifecycleState::PartialRemoteUnknown => ReconcileAction::ConfirmMissingOrEscalate,
        OrderLifecycleState::Failed => ReconcileAction::OperatorRequired,
        _ => ReconcileAction::Noop,
    }
}

pub fn classify_order_lifecycle_divergence(
    local: &OrderLifecycleState,
    remote: RemoteOrderObservation,
) -> OrderLifecycleDivergence {
    match (local, remote) {
        (OrderLifecycleState::RemoteUnknown, RemoteOrderObservation::Open)
        | (OrderLifecycleState::PartialRemoteUnknown, RemoteOrderObservation::Open) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteOpen,
                event: Some(OrderEventKind::ReconcileOpen),
                operator_required: false,
                no_remote_side_effect: true,
                reason: "remote order is open; restore local lifecycle to posted".into(),
            }
        }
        (OrderLifecycleState::RemoteUnknown, RemoteOrderObservation::Missing) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing,
                event: Some(OrderEventKind::ReconcileMissing),
                operator_required: false,
                no_remote_side_effect: true,
                reason: "first missing observation escalates to partial remote unknown".into(),
            }
        }
        (OrderLifecycleState::PartialRemoteUnknown, RemoteOrderObservation::Missing) => {
            OrderLifecycleDivergence {
                kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing,
                event: Some(OrderEventKind::ReconcileMissing),
                operator_required: true,
                no_remote_side_effect: true,
                reason: "second missing observation escalates to operator-required failed state"
                    .into(),
            }
        }
        (
            OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown,
            RemoteOrderObservation::Unknown,
        ) => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::LocalRemoteUnknownStillUnknown,
            event: Some(OrderEventKind::ReconcileUnknown),
            operator_required: true,
            no_remote_side_effect: true,
            reason: "remote truth remains unknown; operator review required".into(),
        },
        (
            OrderLifecycleState::Filled
            | OrderLifecycleState::CancelConfirmed
            | OrderLifecycleState::Failed,
            RemoteOrderObservation::Open,
        ) => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::TerminalLocalRemoteMismatch,
            event: None,
            operator_required: true,
            no_remote_side_effect: true,
            reason: "terminal local state conflicts with open remote observation".into(),
        },
        _ => OrderLifecycleDivergence {
            kind: OrderLifecycleDivergenceKind::None,
            event: None,
            operator_required: false,
            no_remote_side_effect: true,
            reason: "no lifecycle divergence requiring a local transition".into(),
        },
    }
}
