use crate::{
    AdapterCredentialSnapshot, ENV_ALLOW_LIVE_SUBMIT, ENV_ALLOW_SIGN_ONLY_DRY_RUN,
    ENV_RUN_AUTHENTICATED_SMOKE, ENV_RUN_SIGN_ONLY_DRY_RUN, LiveCanaryPreconditions,
    LiveCanaryPrepDecision, LiveCanaryPrepInput, OfficialSdkAdapterConfig, OfficialSdkAdapterError,
    env_flag,
};

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

pub fn validate_read_only_smoke_environment(
    _credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    // Read-only smoke must construct an unauthenticated SDK client and must not consume ambient
    // credentials even when a developer shell has `.env` exported. Credential presence is therefore
    // not a failure by itself; tests must prove the read-only code path does not authenticate, sign,
    // post, cancel, or update remote state.
    Ok(())
}

pub fn validate_authenticated_non_trading_smoke(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if !config.allow_authenticated_non_trading_smoke || !env_flag(ENV_RUN_AUTHENTICATED_SMOKE) {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_AUTHENTICATED_SMOKE}=1 and config.allow_authenticated_non_trading_smoke=true"
        )));
    }
    if !credentials.has_authenticated_material() {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "authenticated non-trading smoke needs L1 or complete L2 credentials".into(),
        ));
    }
    Ok(())
}

pub fn validate_sign_only_dry_run(
    config: &OfficialSdkAdapterConfig,
    credentials: &AdapterCredentialSnapshot,
) -> Result<(), OfficialSdkAdapterError> {
    if config.allow_live_submit || env_flag(ENV_ALLOW_LIVE_SUBMIT) || cfg!(feature = "live-submit")
    {
        return Err(OfficialSdkAdapterError::SafetyGate(
            "sign-only dry-run must not run in a live-submit-enabled process".into(),
        ));
    }
    if !config.allow_sign_only_dry_run
        || !env_flag(ENV_RUN_SIGN_ONLY_DRY_RUN)
        || !env_flag(ENV_ALLOW_SIGN_ONLY_DRY_RUN)
    {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "set {ENV_RUN_SIGN_ONLY_DRY_RUN}=1, {ENV_ALLOW_SIGN_ONLY_DRY_RUN}=1 and config.allow_sign_only_dry_run=true"
        )));
    }
    if !credentials.has_l1_private_key {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "sign-only dry-run needs an L1 signer, but must not post the order".into(),
        ));
    }
    Ok(())
}

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
