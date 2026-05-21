use super::*;

fn sha256_fixture(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn approval_fixture() -> RealFundsCanaryApproval {
    RealFundsCanaryApproval {
        approval_id: "approval-real-funds-canary-1".into(),
        approval_hash: sha256_fixture('a'),
        account_id: AccountId("acct-canary".into()),
        scope: "REAL_FUNDS_CANARY".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        artifact_sha256: sha256_fixture('b'),
        evidence_manifest_sha256: sha256_fixture('c'),
        max_order_notional_usd: "1".into(),
        max_daily_notional_usd: "5".into(),
        execution_style: "FOK_LIMIT_FILL".into(),
        operator_identity_ref: "operator-local-approval".into(),
    }
}

fn risk_limits_fixture() -> RealFundsCanaryRiskLimits {
    RealFundsCanaryRiskLimits {
        max_order_notional_usd: "1".into(),
        max_daily_notional_usd: "5".into(),
        daily_used_notional_usd: "0".into(),
    }
}

fn request_fixture(
    preconditions: RealFundsCanaryPreconditions,
    market: RealFundsCanaryMarketSelection,
) -> RealFundsCanaryRequest {
    RealFundsCanaryRequest {
        account_id: AccountId("acct-canary".into()),
        execution_id: ExecutionId("exec-real-funds-canary-1".into()),
        plan_hash: HashValue("plan-hash-real-funds-canary".into()),
        idempotency_key: "real-funds-canary-idempotency-1".into(),
        approval: approval_fixture(),
        risk_limits: risk_limits_fixture(),
        market,
        preconditions,
    }
}

#[test]
fn real_funds_canary_requires_extra_env_config_approval_and_market_gates() {
    let market = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("safe market candidate should be selected");
    let approval = approval_fixture();
    let risk_limits = risk_limits_fixture();
    let artifact_sha256 = sha256_fixture('b');
    let evidence_manifest_sha256 = sha256_fixture('c');
    let preconditions =
        build_real_funds_canary_preconditions(BuildRealFundsCanaryPreconditionsInput {
            approval: &approval,
            risk_limits: &risk_limits,
            market: &market,
            live_canary: all_live_canary_preconditions(),
            artifact_sha256: &artifact_sha256,
            evidence_manifest_sha256: &evidence_manifest_sha256,
            config_allow_real_funds_canary: false,
            balance_allowance_checked: false,
            selected_market_safe: false,
        });
    let request = request_fixture(preconditions, market);
    let err =
        validate_real_funds_canary_preconditions(&OfficialSdkAdapterConfig::default(), &request)
            .expect_err("real funds canary must be default blocked");
    let error = err.to_string();
    assert!(error.contains("config.allow_real_funds_canary is not enabled"));
    assert!(error.contains("real funds config gate not represented in preconditions"));
    assert!(error.contains("balance/allowance check missing"));
    assert!(error.contains("selected market is not canary safe"));
}

#[test]
fn real_funds_market_selector_picks_highest_safe_liquidity_candidate() {
    let selected = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("selector should choose a safe high-liquidity market");
    assert_eq!(selected.market_id, "market-safe-high");
    assert_eq!(selected.limit_price, "0.50");
    // The canary FOK BUY path submits this as a USDC market-order amount.
    assert_eq!(selected.size, "1");
    assert_eq!(selected.notional_usd, "1");
    assert!(selected.selection_reason.contains("highest liquidity"));
}

#[test]
fn real_funds_market_selector_uses_price_times_ask_size_for_depth() {
    let candidates = vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-shares-not-enough-notional".into(),
            token_id: "123".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            ask_size: "2".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            liquidity_score: 999,
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-enough-notional".into(),
            token_id: "456".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            ask_size: "5".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            liquidity_score: 1,
        },
    ];
    let selected = select_real_funds_canary_market(&candidates, "1")
        .expect("second candidate has enough ask notional");
    assert_eq!(selected.market_id, "market-enough-notional");
}

#[test]
fn real_funds_canary_caps_fail_closed() {
    let approval = approval_fixture();
    let risk_limits = RealFundsCanaryRiskLimits {
        daily_used_notional_usd: "4.50".into(),
        ..risk_limits_fixture()
    };
    let market = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("safe market candidate should be selected");
    let preconditions =
        build_real_funds_canary_preconditions(BuildRealFundsCanaryPreconditionsInput {
            approval: &approval,
            risk_limits: &risk_limits,
            market: &market,
            live_canary: all_live_canary_preconditions(),
            artifact_sha256: &approval.artifact_sha256,
            evidence_manifest_sha256: &approval.evidence_manifest_sha256,
            config_allow_real_funds_canary: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
        });
    assert!(preconditions.max_order_notional_ok);
    assert!(!preconditions.max_daily_notional_ok);
    assert!(!preconditions.live_canary.daily_cap_ok);
}

#[test]
fn real_funds_canary_release_decision_gate_fails_closed() {
    let approval = approval_fixture();
    let decision = ReviewedRealFundsCanaryReleaseDecision {
        decision_id: "decision-1".into(),
        scope: "REAL_FUNDS_CANARY".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        artifact_sha256: approval.artifact_sha256.clone(),
        evidence_manifest_sha256: approval.evidence_manifest_sha256.clone(),
        allow_real_funds_canary: false,
        reviewed_release_decision_present: true,
        operator_identity_ref: "operator-release-review".into(),
    };
    let err = validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
    )
    .expect_err("release decision must explicitly allow real-funds canary");
    assert!(
        err.to_string()
            .contains("real-funds canary not allowed by release decision")
    );
}

#[test]
fn real_funds_canary_release_decision_binds_hashes() {
    let approval = approval_fixture();
    let decision = ReviewedRealFundsCanaryReleaseDecision {
        decision_id: "decision-1".into(),
        scope: "REAL_FUNDS_CANARY".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        artifact_sha256: approval.artifact_sha256.clone(),
        evidence_manifest_sha256: approval.evidence_manifest_sha256.clone(),
        allow_real_funds_canary: true,
        reviewed_release_decision_present: true,
        operator_identity_ref: "operator-release-review".into(),
    };
    validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
    )
    .expect("matching reviewed release decision should pass the release gate");
}

#[test]
fn real_funds_canary_rejects_unsafe_market_candidates() {
    let candidates = vec![RealFundsCanaryMarketCandidate {
        market_id: "market-wide-spread".into(),
        token_id: "123".into(),
        active: true,
        accepting_orders: true,
        closed: false,
        archived: false,
        best_ask: "0.50".into(),
        ask_size: "10".into(),
        spread_bps: 251,
        min_order_size: "1".into(),
        liquidity_score: 999,
    }];
    let err = select_real_funds_canary_market(&candidates, "1")
        .expect_err("unsafe market candidates must fail closed");
    assert!(
        err.to_string()
            .contains("no high-liquidity market candidate")
    );
}

#[test]
fn real_funds_market_diagnostics_are_aggregate_and_fail_closed() {
    let candidates = vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-wide-spread".into(),
            token_id: "123".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.50".into(),
            ask_size: "10".into(),
            spread_bps: 251,
            min_order_size: "1".into(),
            liquidity_score: 999,
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-min-order".into(),
            token_id: "456".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.49".into(),
            ask_size: "0.25".into(),
            spread_bps: 50,
            min_order_size: "2".into(),
            liquidity_score: 1,
        },
    ];
    let discovery = select_real_funds_canary_market_with_diagnostics(&candidates, "1");
    assert!(discovery.selection.is_none());
    assert_eq!(discovery.diagnostics.candidates_seen, 2);
    assert_eq!(discovery.diagnostics.safe_candidates, 0);
    assert_eq!(discovery.diagnostics.max_ask_size.as_deref(), Some("10"));
    assert_eq!(discovery.diagnostics.min_spread_bps, Some(50));
    assert!(discovery.diagnostics.min_order_size_blocks);
    assert_eq!(discovery.diagnostics.rejection_counts.spread_too_wide, 1);
    assert_eq!(
        discovery.diagnostics.rejection_counts.insufficient_ask_size,
        1
    );
    assert_eq!(
        discovery.diagnostics.rejection_counts.min_order_above_cap,
        1
    );

    let rendered = serde_json::to_string(&discovery.diagnostics).expect("render diagnostics");
    assert!(!rendered.contains("123"));
    assert!(!rendered.contains("456"));
    assert!(!rendered.contains("market-wide-spread"));
}

fn safe_market_candidates() -> Vec<RealFundsCanaryMarketCandidate> {
    vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-inactive".into(),
            token_id: "111".into(),
            active: false,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.50".into(),
            ask_size: "10".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            liquidity_score: 1_000,
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-safe-low".into(),
            token_id: "222".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.51".into(),
            ask_size: "20".into(),
            spread_bps: 20,
            min_order_size: "1".into(),
            liquidity_score: 100,
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-safe-high".into(),
            token_id: "333".into(),
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.50".into(),
            ask_size: "20".into(),
            spread_bps: 15,
            min_order_size: "1".into(),
            liquidity_score: 500,
        },
    ]
}
