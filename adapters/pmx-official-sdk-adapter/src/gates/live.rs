use crate::{ENV_ALLOW_LIVE_SUBMIT, OfficialSdkAdapterConfig, OfficialSdkAdapterError, env_flag};

pub fn validate_live_submit_preconditions(
    config: &OfficialSdkAdapterConfig,
    kill_switch_open: bool,
    has_repository_reservation: bool,
    reconcile_worker_healthy: bool,
) -> Result<(), OfficialSdkAdapterError> {
    if !cfg!(feature = "live-submit")
        || !env_flag(ENV_ALLOW_LIVE_SUBMIT)
        || !config.allow_live_submit
    {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "live submit requires live-submit feature, PMX_ALLOW_LIVE_SUBMIT=1 and config.allow_live_submit=true".into(),
        ));
    }
    if config.require_kill_switch_open_for_live_submit && !kill_switch_open {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "kill switch is not explicitly open".into(),
        ));
    }
    if config.require_repository_reservation_for_live_submit && !has_repository_reservation {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "repository reservation is missing".into(),
        ));
    }
    if config.require_reconcile_worker_for_live_submit && !reconcile_worker_healthy {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "reconcile worker is not healthy".into(),
        ));
    }
    Ok(())
}
