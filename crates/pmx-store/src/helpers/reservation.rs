use pmx_core::{QuantityBound, ReservationState, SubmitStatus};

use crate::StoreError;

pub(crate) fn quantity_bound_to_resource_and_amount(
    bound: &QuantityBound,
) -> Result<(&'static str, &str), StoreError> {
    match bound {
        QuantityBound::WorstCaseQuoteNotional(v) => Ok(("worst_case_quote_notional", &v.0)),
        QuantityBound::WorstCaseBaseShares(v) => Ok(("worst_case_base_shares", &v.0)),
        QuantityBound::Unsupported(reason) => Err(StoreError::Conflict(format!(
            "unsupported quantity bound for reservation: {reason}"
        ))),
    }
}

pub(crate) fn reservation_state_to_str(state: &ReservationState) -> &'static str {
    match state {
        ReservationState::Pending => "PENDING",
        ReservationState::Active => "ACTIVE",
        ReservationState::Released => "RELEASED",
        ReservationState::Consumed => "CONSUMED",
        ReservationState::Orphaned => "ORPHANED",
    }
}

pub fn submit_status_str(status: &SubmitStatus) -> &'static str {
    match status {
        SubmitStatus::Accepted => "ACCEPTED",
        SubmitStatus::Posted => "POSTED",
        SubmitStatus::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        SubmitStatus::RemoteUnknown => "REMOTE_UNKNOWN",
        SubmitStatus::Rejected => "REJECTED",
        SubmitStatus::Blocked => "BLOCKED",
    }
}
