use chrono::{DateTime, Utc};

use crate::{
    ENV_ALLOW_REAL_FUNDS_CANARY, OfficialSdkAdapterConfig, OfficialSdkAdapterError,
    RealFundsCanaryApproval, RealFundsCanaryMarketCandidate, RealFundsCanaryMarketSelection,
    RealFundsCanaryPreconditions, RealFundsCanaryRequest, RealFundsCanaryRiskLimits, env_flag,
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
    let candidate = candidates
        .iter()
        .filter(|candidate| market_candidate_is_safe(candidate, max_notional_usd))
        .max_by_key(|candidate| candidate.liquidity_score)
        .ok_or_else(|| {
            OfficialSdkAdapterError::SafetyGate(
                "no high-liquidity market candidate satisfied real funds canary constraints".into(),
            )
        })?;
    Ok(RealFundsCanaryMarketSelection {
        market_id: candidate.market_id.clone(),
        token_id: candidate.token_id.clone(),
        limit_price: candidate.best_ask.clone(),
        size: max_notional_usd.to_string(),
        notional_usd: max_notional_usd.to_string(),
        selection_reason:
            "highest liquidity candidate within active/accepting/spread/depth constraints".into(),
    })
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
        && decimal_gte(&candidate.ask_size, max_notional_usd)
        && decimal_lte(&candidate.min_order_size, max_notional_usd)
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

fn decimal_gte(left: &str, right: &str) -> bool {
    match (parse_decimal(left), parse_decimal(right)) {
        (Some(left), Some(right)) => left >= right,
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
