use pmx_core::{OrderEventKind, OrderLifecycleState};

use crate::StoreError;

pub(crate) fn order_lifecycle_state_to_str(state: &OrderLifecycleState) -> &'static str {
    match state {
        OrderLifecycleState::Planned => "PLANNED",
        OrderLifecycleState::Signed => "SIGNED",
        OrderLifecycleState::PostRequested => "POST_REQUESTED",
        OrderLifecycleState::Posted => "POSTED",
        OrderLifecycleState::PartiallyFilled => "PARTIALLY_FILLED",
        OrderLifecycleState::Filled => "FILLED",
        OrderLifecycleState::CancelRequested => "CANCEL_REQUESTED",
        OrderLifecycleState::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderLifecycleState::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderLifecycleState::ReplaceRequested => "REPLACE_REQUESTED",
        OrderLifecycleState::ReplacementPrepared => "REPLACEMENT_PREPARED",
        OrderLifecycleState::ReplaceCancelPending => "REPLACE_CANCEL_PENDING",
        OrderLifecycleState::ReplaceCancelUnknown => "REPLACE_CANCEL_UNKNOWN",
        OrderLifecycleState::Replaced => "REPLACED",
        OrderLifecycleState::ReplaceRejected => "REPLACE_REJECTED",
        OrderLifecycleState::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderLifecycleState::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        OrderLifecycleState::Failed => "FAILED",
    }
}

pub(crate) fn order_lifecycle_state_from_str(
    value: &str,
) -> Result<OrderLifecycleState, StoreError> {
    match value {
        "PLANNED" => Ok(OrderLifecycleState::Planned),
        "SIGNED" => Ok(OrderLifecycleState::Signed),
        "POST_REQUESTED" => Ok(OrderLifecycleState::PostRequested),
        "POSTED" => Ok(OrderLifecycleState::Posted),
        "PARTIALLY_FILLED" => Ok(OrderLifecycleState::PartiallyFilled),
        "FILLED" => Ok(OrderLifecycleState::Filled),
        "CANCEL_REQUESTED" => Ok(OrderLifecycleState::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderLifecycleState::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderLifecycleState::CancelConfirmed),
        "REPLACE_REQUESTED" => Ok(OrderLifecycleState::ReplaceRequested),
        "REPLACEMENT_PREPARED" => Ok(OrderLifecycleState::ReplacementPrepared),
        "REPLACE_CANCEL_PENDING" => Ok(OrderLifecycleState::ReplaceCancelPending),
        "REPLACE_CANCEL_UNKNOWN" => Ok(OrderLifecycleState::ReplaceCancelUnknown),
        "REPLACED" => Ok(OrderLifecycleState::Replaced),
        "REPLACE_REJECTED" => Ok(OrderLifecycleState::ReplaceRejected),
        "REMOTE_UNKNOWN" => Ok(OrderLifecycleState::RemoteUnknown),
        "PARTIAL_REMOTE_UNKNOWN" => Ok(OrderLifecycleState::PartialRemoteUnknown),
        "FAILED" => Ok(OrderLifecycleState::Failed),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle state: {other}"
        ))),
    }
}

pub(crate) fn order_event_kind_to_str(event: &OrderEventKind) -> &'static str {
    match event {
        OrderEventKind::Signed => "SIGNED",
        OrderEventKind::PostRequested => "POST_REQUESTED",
        OrderEventKind::RemotePosted => "REMOTE_POSTED",
        OrderEventKind::RemoteRejected => "REMOTE_REJECTED",
        OrderEventKind::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderEventKind::PartialFill => "PARTIAL_FILL",
        OrderEventKind::FullFill => "FULL_FILL",
        OrderEventKind::CancelRequested => "CANCEL_REQUESTED",
        OrderEventKind::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderEventKind::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderEventKind::ReplaceRequested => "REPLACE_REQUESTED",
        OrderEventKind::ReplacementPrepared => "REPLACEMENT_PREPARED",
        OrderEventKind::ReplaceCancelRequested => "REPLACE_CANCEL_REQUESTED",
        OrderEventKind::ReplaceCancelUnknown => "REPLACE_CANCEL_UNKNOWN",
        OrderEventKind::ReplacementActivated => "REPLACEMENT_ACTIVATED",
        OrderEventKind::ReplaceRejected => "REPLACE_REJECTED",
        OrderEventKind::ReconcileOpen => "RECONCILE_OPEN",
        OrderEventKind::ReconcileMissing => "RECONCILE_MISSING",
        OrderEventKind::ReconcileUnknown => "RECONCILE_UNKNOWN",
    }
}

pub(crate) fn order_event_kind_from_str(value: &str) -> Result<OrderEventKind, StoreError> {
    match value {
        "SIGNED" => Ok(OrderEventKind::Signed),
        "POST_REQUESTED" => Ok(OrderEventKind::PostRequested),
        "REMOTE_POSTED" => Ok(OrderEventKind::RemotePosted),
        "REMOTE_REJECTED" => Ok(OrderEventKind::RemoteRejected),
        "REMOTE_UNKNOWN" => Ok(OrderEventKind::RemoteUnknown),
        "PARTIAL_FILL" => Ok(OrderEventKind::PartialFill),
        "FULL_FILL" => Ok(OrderEventKind::FullFill),
        "CANCEL_REQUESTED" => Ok(OrderEventKind::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderEventKind::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderEventKind::CancelConfirmed),
        "REPLACE_REQUESTED" => Ok(OrderEventKind::ReplaceRequested),
        "REPLACEMENT_PREPARED" => Ok(OrderEventKind::ReplacementPrepared),
        "REPLACE_CANCEL_REQUESTED" => Ok(OrderEventKind::ReplaceCancelRequested),
        "REPLACE_CANCEL_UNKNOWN" => Ok(OrderEventKind::ReplaceCancelUnknown),
        "REPLACEMENT_ACTIVATED" => Ok(OrderEventKind::ReplacementActivated),
        "REPLACE_REJECTED" => Ok(OrderEventKind::ReplaceRejected),
        "RECONCILE_OPEN" => Ok(OrderEventKind::ReconcileOpen),
        "RECONCILE_MISSING" => Ok(OrderEventKind::ReconcileMissing),
        "RECONCILE_UNKNOWN" => Ok(OrderEventKind::ReconcileUnknown),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle event: {other}"
        ))),
    }
}
