use crate::{
    ENV_ALLOW_LIVE_SUBMIT, LiveCanaryPreconditions, OfficialSdkAdapterConfig,
    OfficialSdkAdapterError, env_flag, validate_live_submit_canary_preconditions,
};

pub fn validate_live_submit_preconditions(
    config: &OfficialSdkAdapterConfig,
    kill_switch_open: bool,
    has_repository_reservation: bool,
    reconcile_worker_healthy: bool,
) -> Result<(), OfficialSdkAdapterError> {
    validate_live_submit_preconditions_with_canary(
        config,
        &LiveCanaryPreconditions {
            compile_feature_live_submit: cfg!(feature = "live-submit"),
            env_allow_live_submit: env_flag(ENV_ALLOW_LIVE_SUBMIT),
            config_allow_live_submit: config.allow_live_submit,
            kill_switch_open: !config.require_kill_switch_open_for_live_submit || kill_switch_open,
            runtime_worker_healthy: false,
            geoblock_allowed: false,
            repository_reservation_exists: !config.require_repository_reservation_for_live_submit
                || has_repository_reservation,
            idempotency_key_written: false,
            reconcile_worker_healthy: !config.require_reconcile_worker_for_live_submit
                || reconcile_worker_healthy,
            account_whitelisted: false,
            market_whitelisted: false,
            size_cap_ok: false,
            daily_cap_ok: false,
            operator_approved: false,
            cancel_only_fallback_ready: false,
        },
    )
}

pub fn validate_live_submit_preconditions_with_canary(
    config: &OfficialSdkAdapterConfig,
    preconditions: &LiveCanaryPreconditions,
) -> Result<(), OfficialSdkAdapterError> {
    if preconditions.config_allow_live_submit != config.allow_live_submit {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "live submit canary config mismatch: preconditions.config_allow_live_submit must match config.allow_live_submit".into(),
        ));
    }
    validate_live_submit_canary_preconditions(preconditions)
}
