use pmx_core::{LiveReadErrorCategory, LiveReadOperation, LiveReadOutcome};

use crate::{LiveReadEventRecord, StoreError};

pub(crate) fn sanitize_live_read_event(mut event: LiveReadEventRecord) -> LiveReadEventRecord {
    event.event_id = None;
    event.observed_at = None;
    event
}

pub(crate) fn validate_live_read_event_for_store(
    event: &LiveReadEventRecord,
) -> Result<(), StoreError> {
    if !event.no_trading_side_effect {
        return Err(StoreError::Conflict(
            "live-read event must be no-trading-side-effect".into(),
        ));
    }
    Ok(())
}

pub(crate) fn live_read_operation_to_str(operation: &LiveReadOperation) -> &'static str {
    match operation {
        LiveReadOperation::GetOrder => "GET_ORDER",
        LiveReadOperation::ListOpenOrders => "LIST_OPEN_ORDERS",
        LiveReadOperation::ListFills => "LIST_FILLS",
        LiveReadOperation::ListPositions => "LIST_POSITIONS",
    }
}

pub(crate) fn live_read_operation_from_str(value: &str) -> Result<LiveReadOperation, StoreError> {
    match value {
        "GET_ORDER" => Ok(LiveReadOperation::GetOrder),
        "LIST_OPEN_ORDERS" => Ok(LiveReadOperation::ListOpenOrders),
        "LIST_FILLS" => Ok(LiveReadOperation::ListFills),
        "LIST_POSITIONS" => Ok(LiveReadOperation::ListPositions),
        other => Err(StoreError::InvalidData(format!(
            "unknown live-read operation {other}"
        ))),
    }
}

pub(crate) fn live_read_outcome_to_str(outcome: &LiveReadOutcome) -> &'static str {
    match outcome {
        LiveReadOutcome::Observed => "OBSERVED",
        LiveReadOutcome::Missing => "MISSING",
        LiveReadOutcome::Blocked => "BLOCKED",
        LiveReadOutcome::RemoteRejected => "REMOTE_REJECTED",
        LiveReadOutcome::RemoteUnknown => "REMOTE_UNKNOWN",
        LiveReadOutcome::AuthenticationFailed => "AUTHENTICATION_FAILED",
    }
}

pub(crate) fn live_read_outcome_from_str(value: &str) -> Result<LiveReadOutcome, StoreError> {
    match value {
        "OBSERVED" => Ok(LiveReadOutcome::Observed),
        "MISSING" => Ok(LiveReadOutcome::Missing),
        "BLOCKED" => Ok(LiveReadOutcome::Blocked),
        "REMOTE_REJECTED" => Ok(LiveReadOutcome::RemoteRejected),
        "REMOTE_UNKNOWN" => Ok(LiveReadOutcome::RemoteUnknown),
        "AUTHENTICATION_FAILED" => Ok(LiveReadOutcome::AuthenticationFailed),
        other => Err(StoreError::InvalidData(format!(
            "unknown live-read outcome {other}"
        ))),
    }
}

pub(crate) fn live_read_error_category_to_str(category: &LiveReadErrorCategory) -> &'static str {
    match category {
        LiveReadErrorCategory::RemoteRejected => "REMOTE_REJECTED",
        LiveReadErrorCategory::RemoteUnknown => "REMOTE_UNKNOWN",
        LiveReadErrorCategory::AuthenticationFailed => "AUTHENTICATION_FAILED",
        LiveReadErrorCategory::Disabled => "DISABLED",
        LiveReadErrorCategory::SigningUnavailable => "SIGNING_UNAVAILABLE",
    }
}

pub(crate) fn live_read_error_category_from_str(
    value: &str,
) -> Result<LiveReadErrorCategory, StoreError> {
    match value {
        "REMOTE_REJECTED" => Ok(LiveReadErrorCategory::RemoteRejected),
        "REMOTE_UNKNOWN" => Ok(LiveReadErrorCategory::RemoteUnknown),
        "AUTHENTICATION_FAILED" => Ok(LiveReadErrorCategory::AuthenticationFailed),
        "DISABLED" => Ok(LiveReadErrorCategory::Disabled),
        "SIGNING_UNAVAILABLE" => Ok(LiveReadErrorCategory::SigningUnavailable),
        other => Err(StoreError::InvalidData(format!(
            "unknown live-read error category {other}"
        ))),
    }
}
