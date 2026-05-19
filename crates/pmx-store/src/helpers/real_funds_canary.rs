use crate::{RealFundsCanaryLifecycleState, StoreError};

pub(crate) fn real_funds_canary_state_to_str(
    state: &RealFundsCanaryLifecycleState,
) -> &'static str {
    match state {
        RealFundsCanaryLifecycleState::PreflightReady => "PREFLIGHT_READY",
        RealFundsCanaryLifecycleState::BlockedPrecheckFailed => "BLOCKED_PRECHECK_FAILED",
        RealFundsCanaryLifecycleState::ReadyButLiveDisabled => "READY_BUT_LIVE_DISABLED",
        RealFundsCanaryLifecycleState::RemoteUnknownFreeze => "REMOTE_UNKNOWN_FREEZE",
        RealFundsCanaryLifecycleState::OperatorRequired => "OPERATOR_REQUIRED",
        RealFundsCanaryLifecycleState::SimulatedReconciled => "SIMULATED_RECONCILED",
    }
}

pub(crate) fn real_funds_canary_state_from_str(
    value: &str,
) -> Result<RealFundsCanaryLifecycleState, StoreError> {
    match value {
        "PREFLIGHT_READY" => Ok(RealFundsCanaryLifecycleState::PreflightReady),
        "BLOCKED_PRECHECK_FAILED" => Ok(RealFundsCanaryLifecycleState::BlockedPrecheckFailed),
        "READY_BUT_LIVE_DISABLED" => Ok(RealFundsCanaryLifecycleState::ReadyButLiveDisabled),
        "REMOTE_UNKNOWN_FREEZE" => Ok(RealFundsCanaryLifecycleState::RemoteUnknownFreeze),
        "OPERATOR_REQUIRED" => Ok(RealFundsCanaryLifecycleState::OperatorRequired),
        "SIMULATED_RECONCILED" => Ok(RealFundsCanaryLifecycleState::SimulatedReconciled),
        other => Err(StoreError::InvalidData(format!(
            "unknown real-funds canary lifecycle state: {other}"
        ))),
    }
}

pub(crate) fn validate_real_funds_canary_transition(
    current: &RealFundsCanaryLifecycleState,
    next: &RealFundsCanaryLifecycleState,
) -> Result<(), StoreError> {
    use RealFundsCanaryLifecycleState::*;

    let allowed = current == next
        || matches!(
            (current, next),
            (PreflightReady, ReadyButLiveDisabled)
                | (PreflightReady, RemoteUnknownFreeze)
                | (PreflightReady, OperatorRequired)
                | (PreflightReady, SimulatedReconciled)
                | (ReadyButLiveDisabled, RemoteUnknownFreeze)
                | (ReadyButLiveDisabled, OperatorRequired)
                | (ReadyButLiveDisabled, SimulatedReconciled)
                | (RemoteUnknownFreeze, OperatorRequired)
        );
    if allowed {
        Ok(())
    } else {
        Err(StoreError::Conflict(format!(
            "invalid real-funds canary transition from {} to {}",
            real_funds_canary_state_to_str(current),
            real_funds_canary_state_to_str(next)
        )))
    }
}
