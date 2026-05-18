use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AccountId, CoreError, ExecutionId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleState {
    Planned,
    ReservationPrepared,
    SigningRequested,
    SignedDryRun,
    Failed,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignOnlyLifecycleEventKind {
    PrepareReservation,
    RequestSigning,
    SignedWithoutPost,
    SigningFailed,
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyLifecycleRecord {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub state: SignOnlyLifecycleState,
    pub event: SignOnlyLifecycleEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_event_id: Option<String>,
    pub signed_order_ref: Option<String>,
    pub no_remote_side_effect: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

pub fn sign_only_lifecycle_records_equivalent(
    left: &SignOnlyLifecycleRecord,
    right: &SignOnlyLifecycleRecord,
) -> bool {
    left.execution_id == right.execution_id
        && left.account_id == right.account_id
        && left.state == right.state
        && left.event == right.event
        && left.client_event_id == right.client_event_id
        && left.signed_order_ref == right.signed_order_ref
        && left.no_remote_side_effect == right.no_remote_side_effect
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderLifecycleState {
    Planned,
    Signed,
    PostRequested,
    Posted,
    PartiallyFilled,
    Filled,
    CancelRequested,
    CancelRemoteAccepted,
    CancelConfirmed,
    RemoteUnknown,
    PartialRemoteUnknown,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderEventKind {
    Signed,
    PostRequested,
    RemotePosted,
    RemoteRejected,
    RemoteUnknown,
    PartialFill,
    FullFill,
    CancelRequested,
    CancelRemoteAccepted,
    CancelConfirmed,
    ReconcileOpen,
    ReconcileMissing,
    ReconcileUnknown,
}

pub fn cancel_state_from_lifecycle(state: &OrderLifecycleState) -> crate::CancelState {
    match state {
        OrderLifecycleState::CancelRequested => crate::CancelState::Requested,
        OrderLifecycleState::CancelRemoteAccepted => crate::CancelState::RemoteAccepted,
        OrderLifecycleState::CancelConfirmed => crate::CancelState::ConfirmedCanceled,
        OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown => {
            crate::CancelState::RemoteUnknown
        }
        OrderLifecycleState::Failed => crate::CancelState::NotCanceled,
        _ => crate::CancelState::ReconcileRequired,
    }
}

pub fn lifecycle_requires_reconcile(state: &OrderLifecycleState) -> bool {
    matches!(
        state,
        OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown
    )
}

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

pub fn transition_order_state(
    from: OrderLifecycleState,
    event: OrderEventKind,
) -> Result<OrderLifecycleState, CoreError> {
    let next = match (&from, &event) {
        (OrderLifecycleState::Planned, OrderEventKind::Signed) => OrderLifecycleState::Signed,
        (OrderLifecycleState::Signed, OrderEventKind::PostRequested) => {
            OrderLifecycleState::PostRequested
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemotePosted) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemoteRejected) => {
            OrderLifecycleState::Failed
        }
        (OrderLifecycleState::PostRequested, OrderEventKind::RemoteUnknown) => {
            OrderLifecycleState::RemoteUnknown
        }
        (OrderLifecycleState::Posted, OrderEventKind::PartialFill) => {
            OrderLifecycleState::PartiallyFilled
        }
        (OrderLifecycleState::Posted, OrderEventKind::FullFill) => OrderLifecycleState::Filled,
        (OrderLifecycleState::PartiallyFilled, OrderEventKind::PartialFill) => {
            OrderLifecycleState::PartiallyFilled
        }
        (OrderLifecycleState::PartiallyFilled, OrderEventKind::FullFill) => {
            OrderLifecycleState::Filled
        }
        (OrderLifecycleState::Posted, OrderEventKind::CancelRequested)
        | (OrderLifecycleState::PartiallyFilled, OrderEventKind::CancelRequested) => {
            OrderLifecycleState::CancelRequested
        }
        (OrderLifecycleState::CancelRequested, OrderEventKind::CancelRemoteAccepted) => {
            OrderLifecycleState::CancelRemoteAccepted
        }
        (OrderLifecycleState::CancelRequested, OrderEventKind::RemoteUnknown)
        | (OrderLifecycleState::CancelRemoteAccepted, OrderEventKind::RemoteUnknown) => {
            OrderLifecycleState::RemoteUnknown
        }
        (OrderLifecycleState::CancelRemoteAccepted, OrderEventKind::CancelConfirmed) => {
            OrderLifecycleState::CancelConfirmed
        }
        (OrderLifecycleState::RemoteUnknown, OrderEventKind::ReconcileOpen) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::RemoteUnknown, OrderEventKind::ReconcileMissing) => {
            OrderLifecycleState::PartialRemoteUnknown
        }
        (OrderLifecycleState::PartialRemoteUnknown, OrderEventKind::ReconcileOpen) => {
            OrderLifecycleState::Posted
        }
        (OrderLifecycleState::PartialRemoteUnknown, OrderEventKind::ReconcileMissing) => {
            OrderLifecycleState::Failed
        }
        (
            OrderLifecycleState::RemoteUnknown | OrderLifecycleState::PartialRemoteUnknown,
            OrderEventKind::ReconcileUnknown,
        ) => from,
        _ => return Err(CoreError::InvalidTransition { from, event }),
    };
    Ok(next)
}

pub fn transition_sign_only_lifecycle(
    from: SignOnlyLifecycleState,
    event: SignOnlyLifecycleEventKind,
) -> Result<SignOnlyLifecycleState, CoreError> {
    let next = match (&from, &event) {
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::PrepareReservation) => {
            SignOnlyLifecycleState::ReservationPrepared
        }
        (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::RequestSigning,
        ) => SignOnlyLifecycleState::SigningRequested,
        (
            SignOnlyLifecycleState::SigningRequested,
            SignOnlyLifecycleEventKind::SignedWithoutPost,
        ) => SignOnlyLifecycleState::SignedDryRun,
        (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::SigningFailed)
        | (
            SignOnlyLifecycleState::ReservationPrepared,
            SignOnlyLifecycleEventKind::SigningFailed,
        ) => SignOnlyLifecycleState::Failed,
        (SignOnlyLifecycleState::Planned, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::ReservationPrepared, SignOnlyLifecycleEventKind::Abandon)
        | (SignOnlyLifecycleState::SigningRequested, SignOnlyLifecycleEventKind::Abandon) => {
            SignOnlyLifecycleState::Abandoned
        }
        _ => return Err(CoreError::InvalidSignOnlyTransition { from, event }),
    };
    Ok(next)
}

pub fn sign_only_lifecycle_has_remote_side_effect(record: &SignOnlyLifecycleRecord) -> bool {
    !record.no_remote_side_effect
}
