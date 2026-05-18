use crate::{LiveCanaryPreconditions, OfficialSdkAdapterError};

pub fn validate_live_submit_canary_preconditions(
    preconditions: &LiveCanaryPreconditions,
) -> Result<(), OfficialSdkAdapterError> {
    let required = [
        (
            preconditions.compile_feature_live_submit,
            "live-submit compile feature disabled",
        ),
        (
            preconditions.env_allow_live_submit,
            "PMX_ALLOW_LIVE_SUBMIT is not enabled",
        ),
        (
            preconditions.config_allow_live_submit,
            "config.allow_live_submit is not enabled",
        ),
        (preconditions.kill_switch_open, "kill switch is not open"),
        (
            preconditions.runtime_worker_healthy,
            "runtime worker is not healthy",
        ),
        (preconditions.geoblock_allowed, "geoblock is not allowed"),
        (
            preconditions.repository_reservation_exists,
            "repository reservation is missing",
        ),
        (
            preconditions.idempotency_key_written,
            "idempotency key is not written",
        ),
        (
            preconditions.reconcile_worker_healthy,
            "reconcile worker is not healthy",
        ),
        (
            preconditions.account_whitelisted,
            "account is not whitelisted",
        ),
        (
            preconditions.market_whitelisted,
            "market is not whitelisted",
        ),
        (preconditions.size_cap_ok, "size cap is exceeded"),
        (preconditions.daily_cap_ok, "daily cap is exceeded"),
        (
            preconditions.operator_approved,
            "operator approval is missing",
        ),
        (
            preconditions.cancel_only_fallback_ready,
            "cancel-only fallback is not ready",
        ),
    ];
    let missing: Vec<_> = required
        .into_iter()
        .filter_map(|(ok, reason)| (!ok).then_some(reason))
        .collect();
    if !missing.is_empty() {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "live submit canary blocked: {}",
            missing.join("; ")
        )));
    }
    Ok(())
}
