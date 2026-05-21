use chrono::{DateTime, Utc};

use crate::{
    ENV_ALLOW_REAL_FUNDS_CANARY, OfficialSdkAdapterConfig, OfficialSdkAdapterError,
    RealFundsCanaryApproval, RealFundsCanaryMarketCandidate, RealFundsCanaryMarketDiagnostics,
    RealFundsCanaryMarketRejectionCounts, RealFundsCanaryMarketSelection,
    RealFundsCanaryMarketValidation, RealFundsCanaryPreconditions, RealFundsCanaryRequest,
    RealFundsCanaryRiskLimits, ReviewedRealFundsCanaryReleaseDecision, env_flag,
    validate_live_submit_canary_preconditions,
};

const REAL_FUNDS_CANARY_SCOPE: &str = "REAL_FUNDS_CANARY";
const REAL_FUNDS_CANARY_EXECUTION_STYLE: &str = "FOK_LIMIT_FILL";
const MAX_SPREAD_BPS: u64 = 250;

pub struct BuildRealFundsCanaryPreconditionsInput<'a> {
    pub approval: &'a RealFundsCanaryApproval,
    pub risk_limits: &'a RealFundsCanaryRiskLimits,
    pub market: &'a RealFundsCanaryMarketSelection,
    pub live_canary: crate::LiveCanaryPreconditions,
    pub artifact_sha256: &'a str,
    pub evidence_manifest_sha256: &'a str,
    pub config_allow_real_funds_canary: bool,
    pub balance_allowance_checked: bool,
    pub selected_market_safe: bool,
}

pub fn validate_real_funds_canary_preconditions(
    config: &OfficialSdkAdapterConfig,
    request: &RealFundsCanaryRequest,
) -> Result<(), OfficialSdkAdapterError> {
    validate_live_submit_canary_preconditions(&request.preconditions.live_canary)?;
    let required = [
        (
            env_flag(ENV_ALLOW_REAL_FUNDS_CANARY),
            "PMX_ALLOW_REAL_FUNDS_CANARY is not enabled",
        ),
        (
            config.allow_real_funds_canary,
            "config.allow_real_funds_canary is not enabled",
        ),
        (
            request.preconditions.env_allow_real_funds_canary,
            "real funds env gate not represented in preconditions",
        ),
        (
            request.preconditions.config_allow_real_funds_canary,
            "real funds config gate not represented in preconditions",
        ),
        (request.preconditions.approval_valid, "approval is invalid"),
        (
            request.preconditions.approval_scope_matches,
            "approval scope mismatch",
        ),
        (
            request.preconditions.approval_not_expired,
            "approval is expired",
        ),
        (
            request.preconditions.artifact_bound,
            "artifact hash is not bound",
        ),
        (
            request.preconditions.evidence_manifest_bound,
            "evidence manifest hash is not bound",
        ),
        (
            request.preconditions.max_order_notional_ok,
            "per-order canary cap exceeded",
        ),
        (
            request.preconditions.max_daily_notional_ok,
            "daily canary cap exceeded",
        ),
        (
            request.preconditions.execution_style_fok_limit_fill,
            "execution style is not FOK limit fill",
        ),
        (
            request.preconditions.balance_allowance_checked,
            "balance/allowance check missing",
        ),
        (
            request.preconditions.selected_market_safe,
            "selected market is not canary safe",
        ),
    ];
    let missing: Vec<_> = required
        .into_iter()
        .filter_map(|(ok, reason)| (!ok).then_some(reason))
        .collect();
    if !missing.is_empty() {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "real funds canary blocked: {}",
            missing.join("; ")
        )));
    }
    Ok(())
}

pub fn validate_reviewed_real_funds_canary_release_decision(
    decision: &ReviewedRealFundsCanaryReleaseDecision,
    approval: &RealFundsCanaryApproval,
    artifact_sha256: &str,
    evidence_manifest_sha256: &str,
) -> Result<(), OfficialSdkAdapterError> {
    let required = [
        (
            !decision.decision_id.trim().is_empty(),
            "decision_id missing",
        ),
        (decision.scope == REAL_FUNDS_CANARY_SCOPE, "scope mismatch"),
        (
            decision.allow_real_funds_canary,
            "real-funds canary not allowed by release decision",
        ),
        (
            decision.reviewed_release_decision_present,
            "reviewed release decision not present",
        ),
        (
            !decision.operator_identity_ref.trim().is_empty(),
            "operator identity ref missing",
        ),
        (
            decision.artifact_sha256 == artifact_sha256
                && decision.artifact_sha256 == approval.artifact_sha256,
            "artifact hash mismatch",
        ),
        (
            decision.evidence_manifest_sha256 == evidence_manifest_sha256
                && decision.evidence_manifest_sha256 == approval.evidence_manifest_sha256,
            "evidence manifest hash mismatch",
        ),
        (
            approval_not_expired(&decision.expires_at),
            "release decision expired",
        ),
    ];
    let missing: Vec<_> = required
        .into_iter()
        .filter_map(|(ok, reason)| (!ok).then_some(reason))
        .collect();
    if !missing.is_empty() {
        return Err(OfficialSdkAdapterError::SafetyGate(format!(
            "real funds canary blocked by release decision gate: {}",
            missing.join("; ")
        )));
    }
    Ok(())
}

pub fn build_real_funds_canary_preconditions(
    input: BuildRealFundsCanaryPreconditionsInput<'_>,
) -> RealFundsCanaryPreconditions {
    let approval_valid = valid_approval(input.approval);
    let mut live_canary = input.live_canary;
    live_canary.size_cap_ok = notional_lte(
        &input.market.notional_usd,
        &input.risk_limits.max_order_notional_usd,
    );
    live_canary.daily_cap_ok = daily_notional_lte(
        &input.risk_limits.daily_used_notional_usd,
        &input.market.notional_usd,
        &input.risk_limits.max_daily_notional_usd,
    );
    RealFundsCanaryPreconditions {
        live_canary,
        env_allow_real_funds_canary: env_flag(ENV_ALLOW_REAL_FUNDS_CANARY),
        config_allow_real_funds_canary: input.config_allow_real_funds_canary,
        approval_valid,
        approval_scope_matches: input.approval.scope == REAL_FUNDS_CANARY_SCOPE,
        approval_not_expired: approval_not_expired(&input.approval.expires_at),
        artifact_bound: is_sha256(input.artifact_sha256)
            && input.approval.artifact_sha256 == input.artifact_sha256,
        evidence_manifest_bound: is_sha256(input.evidence_manifest_sha256)
            && input.approval.evidence_manifest_sha256 == input.evidence_manifest_sha256,
        max_order_notional_ok: notional_lte(
            &input.market.notional_usd,
            &input.risk_limits.max_order_notional_usd,
        ),
        max_daily_notional_ok: daily_notional_lte(
            &input.risk_limits.daily_used_notional_usd,
            &input.market.notional_usd,
            &input.risk_limits.max_daily_notional_usd,
        ),
        execution_style_fok_limit_fill: input.approval.execution_style
            == REAL_FUNDS_CANARY_EXECUTION_STYLE,
        balance_allowance_checked: input.balance_allowance_checked,
        selected_market_safe: input.selected_market_safe,
    }
}

pub fn select_real_funds_canary_market(
    candidates: &[RealFundsCanaryMarketCandidate],
    max_notional_usd: &str,
) -> Result<RealFundsCanaryMarketSelection, OfficialSdkAdapterError> {
    select_real_funds_canary_market_with_diagnostics(candidates, max_notional_usd)
        .selection
        .ok_or_else(|| {
            OfficialSdkAdapterError::SafetyGate(
                "no high-liquidity market candidate satisfied real funds canary constraints".into(),
            )
        })
}

pub fn select_real_funds_canary_market_with_diagnostics(
    candidates: &[RealFundsCanaryMarketCandidate],
    max_notional_usd: &str,
) -> RealFundsCanaryMarketValidation {
    let diagnostics = diagnose_real_funds_canary_markets(candidates, max_notional_usd);
    let selection = candidates
        .iter()
        .filter(|candidate| market_candidate_is_safe(candidate, max_notional_usd))
        .max_by_key(|candidate| candidate.liquidity_score)
        .map(|candidate| RealFundsCanaryMarketSelection {
            market_id: candidate.market_id.clone(),
            token_id: candidate.token_id.clone(),
            limit_price: candidate.best_ask.clone(),
            // For the live FOK BUY path this value is the SDK market-order USDC
            // amount. It is kept in `size` for API compatibility with the
            // existing selection model, while `notional_usd` is the governing
            // risk value.
            size: max_notional_usd.to_string(),
            notional_usd: max_notional_usd.to_string(),
            selection_reason:
                "highest liquidity candidate within active/accepting/spread/min-order/notional-depth constraints"
                    .into(),
        });
    RealFundsCanaryMarketValidation {
        selection,
        diagnostics,
    }
}

pub fn diagnose_real_funds_canary_markets(
    candidates: &[RealFundsCanaryMarketCandidate],
    max_notional_usd: &str,
) -> RealFundsCanaryMarketDiagnostics {
    let mut rejection_counts = RealFundsCanaryMarketRejectionCounts::default();
    let mut safe_candidates = 0;
    let mut max_ask_size: Option<f64> = None;
    let mut min_spread_bps: Option<u64> = None;
    let mut min_order_size_blocks = false;

    for candidate in candidates {
        if let Some(ask_size) = parse_decimal(&candidate.ask_size) {
            max_ask_size = Some(max_ask_size.map_or(ask_size, |current| current.max(ask_size)));
        }
        min_spread_bps = Some(min_spread_bps.map_or(candidate.spread_bps, |current| {
            current.min(candidate.spread_bps)
        }));
        if !candidate.active {
            rejection_counts.inactive += 1;
        }
        if !candidate.accepting_orders {
            rejection_counts.not_accepting_orders += 1;
        }
        if candidate.closed {
            rejection_counts.closed += 1;
        }
        if candidate.archived {
            rejection_counts.archived += 1;
        }
        if candidate.spread_bps > MAX_SPREAD_BPS {
            rejection_counts.spread_too_wide += 1;
        }
        if !decimal_gt_zero(&candidate.best_ask) {
            rejection_counts.missing_or_zero_best_ask += 1;
        }
        if !best_ask_notional_gte(candidate, max_notional_usd) {
            rejection_counts.insufficient_ask_size += 1;
        }
        if !min_order_size_lte_implied_order_size(candidate, max_notional_usd) {
            rejection_counts.min_order_size_above_order_size += 1;
            min_order_size_blocks = true;
        }
        if market_candidate_is_safe(candidate, max_notional_usd) {
            safe_candidates += 1;
        }
    }

    RealFundsCanaryMarketDiagnostics {
        market_validation_complete: true,
        candidates_seen: candidates.len() as u64,
        safe_candidates,
        max_ask_size: max_ask_size.map(format_decimal_summary),
        min_spread_bps,
        min_order_size_blocks,
        rejection_counts,
    }
}

pub fn market_candidate_is_safe(
    candidate: &RealFundsCanaryMarketCandidate,
    max_notional_usd: &str,
) -> bool {
    candidate.active
        && candidate.accepting_orders
        && !candidate.closed
        && !candidate.archived
        && candidate.spread_bps <= MAX_SPREAD_BPS
        && decimal_gt_zero(&candidate.best_ask)
        && best_ask_notional_gte(candidate, max_notional_usd)
        && min_order_size_lte_implied_order_size(candidate, max_notional_usd)
}

fn valid_approval(approval: &RealFundsCanaryApproval) -> bool {
    !approval.approval_id.trim().is_empty()
        && is_sha256(&approval.approval_hash)
        && approval.scope == REAL_FUNDS_CANARY_SCOPE
        && approval.execution_style == REAL_FUNDS_CANARY_EXECUTION_STYLE
        && approval_not_expired(&approval.expires_at)
        && is_sha256(&approval.artifact_sha256)
        && is_sha256(&approval.evidence_manifest_sha256)
        && !approval.operator_identity_ref.trim().is_empty()
        && decimal_gt_zero(&approval.max_order_notional_usd)
        && decimal_gt_zero(&approval.max_daily_notional_usd)
}

fn approval_not_expired(expires_at: &str) -> bool {
    DateTime::parse_from_rfc3339(expires_at)
        .map(|expires_at| expires_at.with_timezone(&Utc) > Utc::now())
        .unwrap_or(false)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn decimal_gt_zero(value: &str) -> bool {
    parse_decimal(value).is_some_and(|value| value > 0.0)
}

fn decimal_lte(left: &str, right: &str) -> bool {
    match (parse_decimal(left), parse_decimal(right)) {
        (Some(left), Some(right)) => left <= right,
        _ => false,
    }
}

fn best_ask_notional_gte(candidate: &RealFundsCanaryMarketCandidate, cap: &str) -> bool {
    match (
        parse_decimal(&candidate.best_ask),
        parse_decimal(&candidate.ask_size),
        parse_decimal(cap),
    ) {
        (Some(price), Some(size), Some(cap)) => price * size >= cap,
        _ => false,
    }
}

fn min_order_size_lte_implied_order_size(
    candidate: &RealFundsCanaryMarketCandidate,
    cap: &str,
) -> bool {
    match (
        parse_decimal(&candidate.min_order_size),
        parse_decimal(cap),
        parse_decimal(&candidate.best_ask),
    ) {
        (Some(min_size), Some(cap), Some(price)) if price > 0.0 => min_size <= cap / price,
        _ => false,
    }
}

fn notional_lte(notional: &str, cap: &str) -> bool {
    decimal_lte(notional, cap)
}

fn daily_notional_lte(used: &str, order: &str, cap: &str) -> bool {
    match (
        parse_decimal(used),
        parse_decimal(order),
        parse_decimal(cap),
    ) {
        (Some(used), Some(order), Some(cap)) => used + order <= cap,
        _ => false,
    }
}

fn parse_decimal(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value || trimmed.starts_with('-') {
        return None;
    }
    let parsed = trimmed.parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}

fn format_decimal_summary(value: f64) -> String {
    let formatted = format!("{value:.8}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}
