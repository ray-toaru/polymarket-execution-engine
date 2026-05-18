use crate::LiveCanaryPreconditions;

pub fn default_blocked_live_canary_preconditions() -> LiveCanaryPreconditions {
    LiveCanaryPreconditions {
        compile_feature_live_submit: false,
        env_allow_live_submit: false,
        config_allow_live_submit: false,
        kill_switch_open: false,
        runtime_worker_healthy: false,
        geoblock_allowed: false,
        repository_reservation_exists: false,
        idempotency_key_written: false,
        reconcile_worker_healthy: false,
        account_whitelisted: false,
        market_whitelisted: false,
        size_cap_ok: false,
        daily_cap_ok: false,
        operator_approved: false,
        cancel_only_fallback_ready: false,
    }
}
