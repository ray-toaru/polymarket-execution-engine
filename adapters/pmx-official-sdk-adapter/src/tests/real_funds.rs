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
        workspace_manifest_sha256: Some(sha256_fixture('e')),
        archived_manifest_sha256: Some(sha256_fixture('c')),
        market_candidate_sha256: sha256_fixture('d'),
        max_order_notional_usd: "1".into(),
        max_daily_notional_usd: "5".into(),
        execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
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

fn exchange_rule_snapshot_fixture(min_share_size: &str) -> ExchangeRuleSnapshot {
    ExchangeRuleSnapshot {
        schema_version: 1,
        venue: "polymarket_clob".into(),
        order_mode: "post_only_limit".into(),
        order_type: "GTC".into(),
        side: "BUY".into(),
        target_size_semantics: "outcome_shares".into(),
        min_share_size: min_share_size.into(),
        min_tick_size: "0.001".into(),
        source: "unit-test-rule-snapshot".into(),
        captured_at: "2099-01-01T00:00:00Z".into(),
        expires_at: "2099-01-01T00:15:00Z".into(),
        evidence_ref: "review://operator/rule-snapshot".into(),
    }
}

fn reviewed_decision_fixture(
    approval: &RealFundsCanaryApproval,
) -> ReviewedRealFundsCanaryReleaseDecision {
    ReviewedRealFundsCanaryReleaseDecision {
        schema_version: 1,
        decision_id: "decision-1".into(),
        status: "reviewed_go".into(),
        source_release: format!("v{}", env!("CARGO_PKG_VERSION")),
        decision: "go".into(),
        decision_reason: "unit test reviewed canary decision".into(),
        scope: "REAL_FUNDS_CANARY".into(),
        execution_style: "GTC_LIMIT_POST_ONLY_CANCEL".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        artifact_sha256: approval.artifact_sha256.clone(),
        evidence_manifest_sha256: approval.evidence_manifest_sha256.clone(),
        workspace_manifest_sha256: approval.workspace_manifest_sha256.clone(),
        archived_manifest_sha256: approval.archived_manifest_sha256.clone(),
        market_candidate_sha256: approval.market_candidate_sha256.clone(),
        github_evidence: serde_json::json!({"root_ci_run_id": "unit-test"}),
        external_references: serde_json::json!({"operator_approval_ref": "approval://unit-test"}),
        risk_limits: serde_json::json!({
            "max_order_notional_usd": "1",
            "target_size": "5",
            "max_daily_notional_usd": "5"
        }),
        required_review_signals: serde_json::json!({"artifact_hash_reviewed": true}),
        live_submit_authorized: true,
        live_cancel_authorized: true,
        production_deployment_authorized: false,
        real_funds_canary_authorized: true,
        remote_side_effects_authorized: true,
        allow_real_funds_canary: true,
        reviewed_release_decision_present: true,
        operator_identity_ref: approval.operator_identity_ref.clone(),
        secrets_included: false,
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
        market_candidate_sha256: sha256_fixture('d'),
        preconditions,
    }
}

#[test]
fn canary_stage_report_marks_remote_side_effect_failures_operator_required() {
    let approval = approval_fixture();
    let risk_limits = risk_limits_fixture();
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
            market_candidate_sha256: &approval.market_candidate_sha256,
            config_allow_real_funds_canary: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: true,
            runtime_live_submit_gate_bound: true,
            runtime_idempotency_lease_bound: true,
            runtime_order_cancel_reconciliation_bound: true,
        });
    let request = request_fixture(preconditions, market);
    let report = RealFundsCanaryStageReport::operator_required(
        &request,
        "cancel_unknown",
        Some("0xposted".into()),
        Some("Live".into()),
        "cancel_order timed out",
    );

    assert_eq!(report.status, "operator_required");
    assert_eq!(report.stage, "cancel_unknown");
    assert_eq!(report.remote_order_id.as_deref(), Some("0xposted"));
    assert!(report.posted);
    assert!(report.remote_side_effects);
    assert!(report.operator_required);
    assert!(!report.raw_signed_order_exposed);
    assert!(
        report
            .error_summary
            .as_deref()
            .expect("error summary")
            .contains("cancel_order timed out")
    );
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
            market_candidate_sha256: &approval.market_candidate_sha256,
            config_allow_real_funds_canary: false,
            balance_allowance_checked: false,
            selected_market_safe: false,
            runtime_kill_switch_truth_bound: false,
            runtime_live_submit_gate_bound: false,
            runtime_idempotency_lease_bound: false,
            runtime_order_cancel_reconciliation_bound: false,
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
fn real_funds_canary_requires_durable_runtime_truth_gates() {
    let market = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("safe market candidate should be selected");
    let approval = approval_fixture();
    let risk_limits = risk_limits_fixture();
    let preconditions =
        build_real_funds_canary_preconditions(BuildRealFundsCanaryPreconditionsInput {
            approval: &approval,
            risk_limits: &risk_limits,
            market: &market,
            live_canary: all_live_canary_preconditions(),
            artifact_sha256: &approval.artifact_sha256,
            evidence_manifest_sha256: &approval.evidence_manifest_sha256,
            market_candidate_sha256: &approval.market_candidate_sha256,
            config_allow_real_funds_canary: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: false,
            runtime_live_submit_gate_bound: false,
            runtime_idempotency_lease_bound: false,
            runtime_order_cancel_reconciliation_bound: false,
        });
    let request = request_fixture(preconditions, market);
    let config = OfficialSdkAdapterConfig {
        allow_real_funds_canary: true,
        clob_host: CLOB_PRODUCTION_HOST.into(),
        ..OfficialSdkAdapterConfig::default()
    };
    let err = validate_real_funds_canary_preconditions(&config, &request)
        .expect_err("missing runtime truth must block real-funds canary");
    let error = err.to_string();
    assert!(error.contains("runtime kill-switch truth missing"));
    assert!(error.contains("runtime live-submit gate truth missing"));
    assert!(error.contains("runtime idempotency lease truth missing"));
    assert!(error.contains("runtime order/cancel reconciliation truth missing"));
}

#[test]
fn real_funds_canary_rejects_legacy_redirecting_clob_host() {
    let market = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("safe market candidate should be selected");
    let approval = approval_fixture();
    let risk_limits = risk_limits_fixture();
    let preconditions =
        build_real_funds_canary_preconditions(BuildRealFundsCanaryPreconditionsInput {
            approval: &approval,
            risk_limits: &risk_limits,
            market: &market,
            live_canary: all_live_canary_preconditions(),
            artifact_sha256: &approval.artifact_sha256,
            evidence_manifest_sha256: &approval.evidence_manifest_sha256,
            market_candidate_sha256: &approval.market_candidate_sha256,
            config_allow_real_funds_canary: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: true,
            runtime_live_submit_gate_bound: true,
            runtime_idempotency_lease_bound: true,
            runtime_order_cancel_reconciliation_bound: true,
        });
    let request = request_fixture(preconditions, market);
    let config = OfficialSdkAdapterConfig {
        allow_real_funds_canary: true,
        clob_host: LEGACY_CLOB_V2_REDIRECT_HOST.into(),
        ..OfficialSdkAdapterConfig::default()
    };
    let err = validate_real_funds_canary_preconditions(&config, &request)
        .expect_err("legacy redirecting host must fail before live posting");
    assert!(
        err.to_string()
            .contains("real funds canary requires canonical CLOB production host")
    );
}

#[test]
fn real_funds_market_selector_picks_highest_safe_liquidity_candidate() {
    let selected = select_real_funds_canary_market(&safe_market_candidates(), "1")
        .expect("selector should choose a safe high-liquidity market");
    assert_eq!(selected.market_id, "market-safe-high");
    assert_eq!(selected.limit_price, "0.19");
    assert_eq!(selected.size, "5");
    assert_eq!(selected.notional_usd, "0.95");
    assert!(selected.selection_reason.contains("highest liquidity"));
}

#[test]
fn real_funds_market_selector_requires_target_size_at_top_ask() {
    let candidates = vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-shares-not-enough-notional".into(),
            token_id: "123".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "2".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 999,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-shares-not-enough-notional".into(),
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-enough-notional".into(),
            token_id: "456".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "5".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 1,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-enough-notional".into(),
        },
    ];
    let selected = select_real_funds_canary_market(&candidates, "1")
        .expect("second candidate has enough ask size");
    assert_eq!(selected.market_id, "market-enough-notional");
    assert_eq!(selected.size, "5");
    assert_eq!(selected.notional_usd, "0.95");
}

#[test]
fn real_funds_market_selector_derives_notional_from_price_times_target_size() {
    let candidates = vec![RealFundsCanaryMarketCandidate {
        market_id: "market-over-cap".into(),
        token_id: "123".into(),
        side: "BUY".into(),
        order_type: "GTC".into(),
        post_only: true,
        active: true,
        accepting_orders: true,
        closed: false,
        archived: false,
        best_ask: "0.30".into(),
        limit_price: "0.29".into(),
        ask_size: "10".into(),
        target_size: "5".into(),
        estimated_order_notional_usd: "1.45".into(),
        spread_bps: 10,
        min_order_size: "5".into(),
        exchange_rule_snapshot: exchange_rule_snapshot_fixture("5"),
        liquidity_score: 999,
        book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
        human_review_ref: "review://operator/market-over-cap".into(),
    }];
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates, "1");
    assert!(diagnostics.selection.is_none());
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.notional_over_cap,
        1
    );
}

#[test]
fn real_funds_market_selector_requires_candidate_notional_binding() {
    let mut candidates = safe_market_candidates();
    candidates[2].estimated_order_notional_usd = "0.94".into();
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates[2..3], "1");
    assert!(diagnostics.selection.is_none());
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .notional_binding_mismatch,
        1
    );
}

#[test]
fn real_funds_market_selector_allows_gtc_post_only_size_five_below_one_dollar() {
    let candidates = vec![RealFundsCanaryMarketCandidate {
        market_id: "market-web-limit-size-ok-but-fok-buy-notional-too-low".into(),
        token_id: "123".into(),
        side: "BUY".into(),
        order_type: "GTC".into(),
        post_only: true,
        active: true,
        accepting_orders: true,
        closed: false,
        archived: false,
        best_ask: "0.024".into(),
        limit_price: "0.023".into(),
        ask_size: "1386.16".into(),
        target_size: "5".into(),
        estimated_order_notional_usd: "0.115".into(),
        spread_bps: 10,
        min_order_size: "5".into(),
        exchange_rule_snapshot: exchange_rule_snapshot_fixture("5"),
        liquidity_score: 999,
        book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
        human_review_ref: "review://operator/web-limit-size-ok-fok-buy-notional-too-low".into(),
    }];
    let selected = select_real_funds_canary_market(&candidates, "1")
        .expect("GTC post-only canary should be governed by size and notional cap, not FOK floor");
    assert_eq!(selected.size, "5");
    assert_eq!(selected.limit_price, "0.023");
    assert_eq!(selected.notional_usd, "0.115");
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates, "1");
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.post_only_not_bound,
        0
    );
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.notional_over_cap,
        0
    );
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .min_order_size_above_order_size,
        0
    );
}

#[test]
fn real_funds_market_selector_rejects_missing_or_stale_rule_snapshot() {
    let mut candidates = safe_market_candidates();
    candidates[2].exchange_rule_snapshot.expires_at = "2000-01-01T00:00:00Z".into();
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates[2..3], "1");
    assert!(diagnostics.selection.is_none());
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .exchange_rule_snapshot_invalid,
        1
    );
}

#[test]
fn real_funds_market_selector_rejects_crossing_post_only_buy_price() {
    let mut candidates = safe_market_candidates();
    candidates[2].limit_price = candidates[2].best_ask.clone();
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates[2..3], "1");
    assert!(diagnostics.selection.is_none());
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.post_only_not_bound,
        1
    );
}

#[test]
fn real_funds_market_selector_rejects_limit_price_off_tick_grid() {
    let mut candidates = safe_market_candidates();
    candidates[2].best_ask = "0.024".into();
    candidates[2].limit_price = "0.014".into();
    candidates[2].exchange_rule_snapshot.min_tick_size = "0.01".into();
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates[2..3], "1");
    assert!(diagnostics.selection.is_none());
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.post_only_not_bound,
        1
    );
}

#[test]
fn real_funds_market_selector_uses_fixed_decimal_for_notional_cap() {
    let candidates = vec![RealFundsCanaryMarketCandidate {
        market_id: "market-decimal-boundary".into(),
        token_id: "123".into(),
        side: "BUY".into(),
        order_type: "GTC".into(),
        post_only: true,
        active: true,
        accepting_orders: true,
        closed: false,
        archived: false,
        best_ask: "0.1".into(),
        limit_price: "0.09".into(),
        ask_size: "1".into(),
        target_size: "0.2".into(),
        estimated_order_notional_usd: "0.018".into(),
        spread_bps: 10,
        min_order_size: "0.1".into(),
        exchange_rule_snapshot: exchange_rule_snapshot_fixture("0.1"),
        liquidity_score: 999,
        book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
        human_review_ref: "review://operator/market-decimal-boundary".into(),
    }];
    let selected = select_real_funds_canary_market(&candidates, "0.02")
        .expect("0.1 * 0.2 should equal the exact cap without binary float drift");
    assert_eq!(selected.notional_usd, "0.018");
}

#[test]
fn real_funds_market_selector_compares_min_order_to_target_size() {
    let candidates = vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-low-price-safe-size".into(),
            token_id: "123".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.006".into(),
            limit_price: "0.005".into(),
            ask_size: "2000".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.025".into(),
            spread_bps: 10,
            min_order_size: "5".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("5"),
            liquidity_score: 10,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-low-price-safe-size".into(),
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-high-price-small-size".into(),
            token_id: "456".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "10".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 10,
            min_order_size: "6".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("6"),
            liquidity_score: 999,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-high-price-small-size".into(),
        },
    ];
    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates, "1");
    assert_eq!(
        diagnostics
            .selection
            .as_ref()
            .map(|selection| selection.market_id.as_str()),
        Some("market-low-price-safe-size")
    );
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .min_order_size_above_order_size,
        1
    );
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.post_only_not_bound,
        0
    );
}

#[test]
fn real_funds_canary_caps_fail_closed() {
    let approval = approval_fixture();
    let risk_limits = RealFundsCanaryRiskLimits {
        daily_used_notional_usd: "4.75".into(),
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
            market_candidate_sha256: &approval.market_candidate_sha256,
            config_allow_real_funds_canary: true,
            balance_allowance_checked: true,
            selected_market_safe: true,
            runtime_kill_switch_truth_bound: true,
            runtime_live_submit_gate_bound: true,
            runtime_idempotency_lease_bound: true,
            runtime_order_cancel_reconciliation_bound: true,
        });
    assert!(preconditions.max_order_notional_ok);
    assert!(!preconditions.max_daily_notional_ok);
    assert!(!preconditions.live_canary.daily_cap_ok);
}

#[test]
fn real_funds_canary_release_decision_gate_fails_closed() {
    let approval = approval_fixture();
    let mut decision = reviewed_decision_fixture(&approval);
    decision.allow_real_funds_canary = false;
    let err = validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
        &approval.market_candidate_sha256,
    )
    .expect_err("release decision must explicitly allow real-funds canary");
    assert!(
        err.to_string()
            .contains("real-funds canary not allowed by release decision")
    );
}

#[test]
fn real_funds_canary_release_decision_rejects_not_active_go_status() {
    let approval = approval_fixture();
    let mut decision = reviewed_decision_fixture(&approval);
    decision.status = "draft_not_approved_not_active".into();
    let err = validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
        &approval.market_candidate_sha256,
    )
    .expect_err("go decisions must require the reviewed_go status");
    assert!(
        err.to_string()
            .contains("release decision status is not reviewed_go")
    );
}

#[test]
fn real_funds_canary_release_decision_binds_hashes() {
    let approval = approval_fixture();
    let decision = reviewed_decision_fixture(&approval);
    validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
        &approval.market_candidate_sha256,
    )
    .expect("matching reviewed release decision should pass the release gate");
}

#[test]
fn real_funds_canary_release_decision_binds_market_candidate_hash() {
    let approval = approval_fixture();
    let mut decision = reviewed_decision_fixture(&approval);
    decision.market_candidate_sha256 = sha256_fixture('e');
    let err = validate_reviewed_real_funds_canary_release_decision(
        &decision,
        &approval,
        &approval.artifact_sha256,
        &approval.evidence_manifest_sha256,
        &approval.market_candidate_sha256,
    )
    .expect_err("release decision must bind the reviewed market candidate hash");
    assert!(err.to_string().contains("market candidate hash mismatch"));
}

#[test]
fn real_funds_canary_rejects_unsafe_market_candidates() {
    let candidates = vec![RealFundsCanaryMarketCandidate {
        market_id: "market-wide-spread".into(),
        token_id: "123".into(),
        side: "BUY".into(),
        order_type: "GTC".into(),
        post_only: true,
        active: true,
        accepting_orders: true,
        closed: false,
        archived: false,
        best_ask: "0.10".into(),
        limit_price: "0.09".into(),
        ask_size: "10".into(),
        target_size: "5".into(),
        estimated_order_notional_usd: "0.45".into(),
        spread_bps: 251,
        min_order_size: "1".into(),
        exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
        liquidity_score: 999,
        book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
        human_review_ref: "review://operator/market-wide-spread".into(),
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
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "10".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 251,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 999,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-wide-spread".into(),
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-min-order".into(),
            token_id: "456".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "10".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 50,
            min_order_size: "6".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("6"),
            liquidity_score: 1,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-min-order".into(),
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
        0
    );
    assert_eq!(
        discovery
            .diagnostics
            .rejection_counts
            .min_order_size_above_order_size,
        1
    );
    assert_eq!(discovery.diagnostics.rejection_counts.notional_over_cap, 0);
    assert_eq!(
        discovery.diagnostics.rejection_counts.post_only_not_bound,
        0
    );

    let rendered = serde_json::to_string(&discovery.diagnostics).expect("render diagnostics");
    assert!(!rendered.contains("123"));
    assert!(!rendered.contains("456"));
    assert!(!rendered.contains("market-wide-spread"));
}

#[test]
fn real_funds_market_selector_requires_buy_gtc_post_only_and_human_review() {
    let mut candidates = safe_market_candidates();
    candidates[2].side = "SELL".into();
    candidates[2].order_type = "FOK".into();
    candidates[2].post_only = false;
    candidates[2].human_review_ref.clear();
    candidates[2].book_snapshot_timestamp.clear();

    let diagnostics = select_real_funds_canary_market_with_diagnostics(&candidates, "1");
    assert_eq!(diagnostics.selection.unwrap().market_id, "market-safe-low");
    assert_eq!(diagnostics.diagnostics.safe_candidates, 1);
    assert_eq!(diagnostics.diagnostics.rejection_counts.wrong_side, 1);
    assert_eq!(diagnostics.diagnostics.rejection_counts.wrong_order_type, 1);
    assert_eq!(
        diagnostics.diagnostics.rejection_counts.post_only_not_bound,
        1
    );
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .missing_human_review_ref,
        1
    );
    assert_eq!(
        diagnostics
            .diagnostics
            .rejection_counts
            .missing_book_snapshot_timestamp,
        1
    );
}

fn safe_market_candidates() -> Vec<RealFundsCanaryMarketCandidate> {
    vec![
        RealFundsCanaryMarketCandidate {
            market_id: "market-inactive".into(),
            token_id: "111".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: false,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.10".into(),
            limit_price: "0.09".into(),
            ask_size: "10".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.45".into(),
            spread_bps: 10,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 1_000,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-inactive".into(),
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-safe-low".into(),
            token_id: "222".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "20".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 20,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 100,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-safe-low".into(),
        },
        RealFundsCanaryMarketCandidate {
            market_id: "market-safe-high".into(),
            token_id: "333".into(),
            side: "BUY".into(),
            order_type: "GTC".into(),
            post_only: true,
            active: true,
            accepting_orders: true,
            closed: false,
            archived: false,
            best_ask: "0.20".into(),
            limit_price: "0.19".into(),
            ask_size: "20".into(),
            target_size: "5".into(),
            estimated_order_notional_usd: "0.95".into(),
            spread_bps: 15,
            min_order_size: "1".into(),
            exchange_rule_snapshot: exchange_rule_snapshot_fixture("1"),
            liquidity_score: 500,
            book_snapshot_timestamp: "2099-01-01T00:00:00Z".into(),
            human_review_ref: "review://operator/market-safe-high".into(),
        },
    ]
}
