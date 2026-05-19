use super::*;

#[test]
fn default_config_cannot_live_submit() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(!config.allow_authenticated_non_trading_smoke);
    assert!(!config.allow_sign_only_dry_run);
    assert!(!config.allow_live_submit);
    assert!(config.require_kill_switch_open_for_live_submit);
    assert!(config.require_repository_reservation_for_live_submit);
    assert!(config.require_reconcile_worker_for_live_submit);
}

#[test]
fn read_only_smoke_ignores_ambient_credentials_but_must_remain_unauthenticated() {
    validate_read_only_smoke_environment(&empty_credentials()).expect("empty credentials allowed");
    validate_read_only_smoke_environment(&l1_credentials())
        .expect("ambient credentials do not fail read-only validation; the code path must remain unauthenticated");
}

#[test]
fn authenticated_non_trading_is_explicit_opt_in() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(validate_authenticated_non_trading_smoke(&config, &l1_credentials()).is_err());
}

#[test]
fn sign_only_is_not_live_submit() {
    let config = OfficialSdkAdapterConfig {
        allow_sign_only_dry_run: true,
        allow_live_submit: true,
        ..OfficialSdkAdapterConfig::default()
    };
    assert!(validate_sign_only_dry_run(&config, &l1_credentials()).is_err());
}

#[test]
fn live_submit_preconditions_are_closed_by_default() {
    let config = OfficialSdkAdapterConfig::default();
    assert!(validate_live_submit_preconditions(&config, true, true, true).is_err());
}

#[test]
fn live_submit_canary_requires_every_gate() {
    let mut preconditions = all_live_canary_preconditions();
    validate_live_submit_canary_preconditions(&preconditions).expect("all gates set");
    preconditions.operator_approved = false;
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("operator approval is mandatory");
    assert!(err.to_string().contains("operator approval is missing"));
}

#[test]
fn live_canary_default_preconditions_are_blocked_without_side_effects() {
    let preconditions = default_blocked_live_canary_preconditions();
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("default live canary preconditions must be blocked");
    assert!(
        err.to_string()
            .contains("live-submit compile feature disabled")
    );
    assert!(
        err.to_string()
            .contains("PMX_ALLOW_LIVE_SUBMIT is not enabled")
    );
    assert!(
        err.to_string()
            .contains("cancel-only fallback is not ready")
    );
}

#[test]
fn live_submit_canary_requires_cancel_only_fallback() {
    let mut preconditions = all_live_canary_preconditions();
    preconditions.cancel_only_fallback_ready = false;
    let err = validate_live_submit_canary_preconditions(&preconditions)
        .expect_err("cancel-only fallback is mandatory");
    assert!(
        err.to_string()
            .contains("cancel-only fallback is not ready")
    );
}

#[test]
fn live_canary_prep_freezes_on_remote_unknown_and_never_submits() {
    let decision = prepare_live_canary_decision(&LiveCanaryPrepInput {
        account_id: "acct-canary".into(),
        market_id: "market-canary".into(),
        order_size_units: 1,
        daily_used_units: 0,
        per_order_cap_units: 10,
        per_day_cap_units: 10,
        account_whitelist: vec!["acct-canary".into()],
        market_whitelist: vec!["market-canary".into()],
        operator_approval_id: Some("approval-1".into()),
        cancel_only_fallback_ready: true,
        remote_unknown_orders: 1,
    });
    assert!(decision.frozen);
    assert!(!decision.submit_allowed);
    assert!(!decision.live_side_effects);
    assert!(
        decision
            .reasons
            .contains(&"remote unknown freeze active".into())
    );
}

#[test]
fn live_canary_prep_requires_whitelist_caps_approval_and_cancel_fallback() {
    let decision = prepare_live_canary_decision(&LiveCanaryPrepInput {
        account_id: "acct-not-allowed".into(),
        market_id: "market-not-allowed".into(),
        order_size_units: 11,
        daily_used_units: 9,
        per_order_cap_units: 10,
        per_day_cap_units: 10,
        account_whitelist: vec!["acct-canary".into()],
        market_whitelist: vec!["market-canary".into()],
        operator_approval_id: None,
        cancel_only_fallback_ready: false,
        remote_unknown_orders: 0,
    });
    assert!(!decision.preconditions.account_whitelisted);
    assert!(!decision.preconditions.market_whitelisted);
    assert!(!decision.preconditions.size_cap_ok);
    assert!(!decision.preconditions.daily_cap_ok);
    assert!(!decision.preconditions.operator_approved);
    assert!(!decision.preconditions.cancel_only_fallback_ready);
    assert!(!decision.submit_allowed);
    assert!(!decision.live_side_effects);
}

fn canary_prep_base_input() -> LiveCanaryPrepInput {
    LiveCanaryPrepInput {
        account_id: "acct-canary".into(),
        market_id: "market-canary".into(),
        order_size_units: 1,
        daily_used_units: 0,
        per_order_cap_units: 10,
        per_day_cap_units: 10,
        account_whitelist: vec!["acct-canary".into()],
        market_whitelist: vec!["market-canary".into()],
        operator_approval_id: Some("approval-1".into()),
        cancel_only_fallback_ready: true,
        remote_unknown_orders: 0,
    }
}

#[test]
fn live_canary_prep_negative_scenarios_fail_closed_individually() {
    let cases = [
        (
            "missing_operator_approval",
            LiveCanaryPrepInput {
                operator_approval_id: None,
                ..canary_prep_base_input()
            },
            "operator approval missing",
        ),
        (
            "per_order_cap_exceeded",
            LiveCanaryPrepInput {
                order_size_units: 11,
                ..canary_prep_base_input()
            },
            "per-order cap exceeded",
        ),
        (
            "per_day_cap_exceeded",
            LiveCanaryPrepInput {
                daily_used_units: 10,
                ..canary_prep_base_input()
            },
            "per-day cap exceeded",
        ),
        (
            "account_not_whitelisted",
            LiveCanaryPrepInput {
                account_id: "acct-other".into(),
                ..canary_prep_base_input()
            },
            "account not whitelisted",
        ),
        (
            "market_not_whitelisted",
            LiveCanaryPrepInput {
                market_id: "market-other".into(),
                ..canary_prep_base_input()
            },
            "market not whitelisted",
        ),
        (
            "cancel_only_fallback_missing",
            LiveCanaryPrepInput {
                cancel_only_fallback_ready: false,
                ..canary_prep_base_input()
            },
            "cancel-only fallback missing",
        ),
        (
            "remote_unknown_freeze",
            LiveCanaryPrepInput {
                remote_unknown_orders: 1,
                ..canary_prep_base_input()
            },
            "remote unknown freeze active",
        ),
    ];

    for (name, input, reason) in cases {
        let decision = prepare_live_canary_decision(&input);
        assert!(!decision.submit_allowed, "{name} must fail closed");
        assert!(
            !decision.live_side_effects,
            "{name} must not execute remotely"
        );
        assert!(
            decision.reasons.iter().any(|candidate| candidate == reason),
            "{name} should record reason {reason}"
        );
    }
}
