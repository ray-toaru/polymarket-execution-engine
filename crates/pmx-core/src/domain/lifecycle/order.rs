use serde::{Deserialize, Serialize};

use crate::CoreError;

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
    ReplaceRequested,
    ReplacementPrepared,
    ReplaceCancelPending,
    ReplaceCancelUnknown,
    Replaced,
    ReplaceRejected,
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
    ReplaceRequested,
    ReplacementPrepared,
    ReplaceCancelRequested,
    ReplaceCancelUnknown,
    ReplacementActivated,
    ReplaceRejected,
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
        (
            OrderLifecycleState::Posted | OrderLifecycleState::PartiallyFilled,
            OrderEventKind::ReplaceRequested,
        ) => OrderLifecycleState::ReplaceRequested,
        (OrderLifecycleState::ReplaceRequested, OrderEventKind::ReplacementPrepared) => {
            OrderLifecycleState::ReplacementPrepared
        }
        (OrderLifecycleState::ReplacementPrepared, OrderEventKind::ReplaceCancelRequested) => {
            OrderLifecycleState::ReplaceCancelPending
        }
        (OrderLifecycleState::ReplaceCancelPending, OrderEventKind::ReplaceCancelUnknown) => {
            OrderLifecycleState::ReplaceCancelUnknown
        }
        (OrderLifecycleState::ReplaceCancelPending, OrderEventKind::ReplacementActivated) => {
            OrderLifecycleState::Replaced
        }
        (
            OrderLifecycleState::ReplaceRequested
            | OrderLifecycleState::ReplacementPrepared
            | OrderLifecycleState::ReplaceCancelPending,
            OrderEventKind::ReplaceRejected,
        ) => OrderLifecycleState::ReplaceRejected,
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
