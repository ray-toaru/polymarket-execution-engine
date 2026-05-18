use crate::{
    ENV_ALLOW_LIVE_SUBMIT, LiveCanaryPreconditions, LiveCanaryPrepDecision, LiveCanaryPrepInput,
    env_flag,
};

pub fn prepare_live_canary_decision(input: &LiveCanaryPrepInput) -> LiveCanaryPrepDecision {
    let account_whitelisted = input.account_whitelist.contains(&input.account_id);
    let market_whitelisted = input.market_whitelist.contains(&input.market_id);
    let size_cap_ok =
        input.order_size_units > 0 && input.order_size_units <= input.per_order_cap_units;
    let daily_cap_ok = input.order_size_units > 0
        && input
            .daily_used_units
            .saturating_add(input.order_size_units)
            <= input.per_day_cap_units;
    let operator_approved = input
        .operator_approval_id
        .as_ref()
        .is_some_and(|approval| !approval.trim().is_empty());
    let frozen = input.remote_unknown_orders > 0;
    let preconditions = LiveCanaryPreconditions {
        compile_feature_live_submit: cfg!(feature = "live-submit"),
        env_allow_live_submit: env_flag(ENV_ALLOW_LIVE_SUBMIT),
        config_allow_live_submit: false,
        kill_switch_open: false,
        runtime_worker_healthy: false,
        geoblock_allowed: false,
        repository_reservation_exists: false,
        idempotency_key_written: false,
        reconcile_worker_healthy: false,
        account_whitelisted,
        market_whitelisted,
        size_cap_ok,
        daily_cap_ok,
        operator_approved,
        cancel_only_fallback_ready: input.cancel_only_fallback_ready,
    };
    let mut reasons = Vec::new();
    if !account_whitelisted {
        reasons.push("account not whitelisted".into());
    }
    if !market_whitelisted {
        reasons.push("market not whitelisted".into());
    }
    if !size_cap_ok {
        reasons.push("per-order cap exceeded".into());
    }
    if !daily_cap_ok {
        reasons.push("per-day cap exceeded".into());
    }
    if !operator_approved {
        reasons.push("operator approval missing".into());
    }
    if !input.cancel_only_fallback_ready {
        reasons.push("cancel-only fallback missing".into());
    }
    if frozen {
        reasons.push("remote unknown freeze active".into());
    }
    LiveCanaryPrepDecision {
        preconditions,
        frozen,
        submit_allowed: false,
        reasons,
        live_side_effects: false,
    }
}
