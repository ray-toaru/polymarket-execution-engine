use chrono::{DateTime, Utc};
use std::cmp::Ordering;

use crate::{
    ENV_ALLOW_REAL_FUNDS_CANARY, ExchangeRuleSnapshot, OfficialSdkAdapterConfig,
    OfficialSdkAdapterError, RealFundsCanaryApproval, RealFundsCanaryMarketCandidate,
    RealFundsCanaryMarketDiagnostics, RealFundsCanaryMarketRejectionCounts,
    RealFundsCanaryMarketSelection, RealFundsCanaryMarketValidation, RealFundsCanaryPreconditions,
    RealFundsCanaryRequest, RealFundsCanaryRiskLimits, ReviewedRealFundsCanaryReleaseDecision,
    env_flag, is_canonical_production_clob_host, validate_live_submit_canary_preconditions,
};

const REAL_FUNDS_CANARY_SCOPE: &str = "REAL_FUNDS_CANARY";
const REAL_FUNDS_CANARY_EXECUTION_STYLE: &str = "GTC_LIMIT_POST_ONLY_CANCEL";
const REAL_FUNDS_CANARY_ORDER_MODE: &str = "post_only_limit";
const REAL_FUNDS_CANARY_SIDE: &str = "BUY";
const REAL_FUNDS_CANARY_ORDER_TYPE: &str = "GTC";
const REAL_FUNDS_CANARY_TARGET_SIZE_SEMANTICS: &str = "outcome_shares";
const MAX_SPREAD_BPS: u64 = 250;

pub struct BuildRealFundsCanaryPreconditionsInput<'a> {
    pub approval: &'a RealFundsCanaryApproval,
    pub risk_limits: &'a RealFundsCanaryRiskLimits,
    pub market: &'a RealFundsCanaryMarketSelection,
    pub live_canary: crate::LiveCanaryPreconditions,
    pub artifact_sha256: &'a str,
    pub evidence_manifest_sha256: &'a str,
    pub market_candidate_sha256: &'a str,
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
            request.preconditions.market_candidate_bound,
            "market candidate hash is not bound",
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
            request.preconditions.execution_style_gtc_post_only_cancel,
            "execution style is not GTC post-only cancel",
        ),
        (
            request.preconditions.balance_allowance_checked,
            "balance/allowance check missing",
        ),
        (
            request.preconditions.selected_market_safe,
            "selected market is not canary safe",
        ),
        (
            is_canonical_production_clob_host(&config.clob_host),
            "real funds canary requires canonical CLOB production host",
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
    market_candidate_sha256: &str,
) -> Result<(), OfficialSdkAdapterError> {
    let source_release = format!("v{}", env!("CARGO_PKG_VERSION"));
    let required = [
        (decision.schema_version == 1, "schema_version mismatch"),
        (
            !decision.decision_id.trim().is_empty(),
            "decision_id missing",
        ),
        (
            decision.status == "reviewed_go",
            "release decision status is not reviewed_go",
        ),
        (
            !decision.decision_reason.trim().is_empty(),
            "decision reason missing",
        ),
        (
            decision.source_release == source_release,
            "source release mismatch",
        ),
        (decision.decision == "go", "release decision is not go"),
        (decision.scope == REAL_FUNDS_CANARY_SCOPE, "scope mismatch"),
        (
            decision.execution_style == REAL_FUNDS_CANARY_EXECUTION_STYLE,
            "execution style mismatch",
        ),
        (
            !decision.github_evidence.is_null(),
            "github evidence missing",
        ),
        (
            !decision.external_references.is_null(),
            "external references missing",
        ),
        (!decision.risk_limits.is_null(), "risk limits missing"),
        (
            !decision.required_review_signals.is_null(),
            "review signals missing",
        ),
        (
            !decision.secrets_included,
            "decision must not include secrets",
        ),
        (
            decision.live_submit_authorized,
            "live submit not authorized by release decision",
        ),
        (
            decision.live_cancel_authorized,
            "canary cancel not authorized by release decision",
        ),
        (
            !decision.production_deployment_authorized,
            "production deployment must remain unauthorized for canary",
        ),
        (
            decision.real_funds_canary_authorized,
            "real-funds canary flag is not authorized",
        ),
        (
            decision.remote_side_effects_authorized,
            "remote side effects not authorized by release decision",
        ),
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
            decision.operator_identity_ref == approval.operator_identity_ref,
            "operator identity ref mismatch",
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
            decision
                .archived_manifest_sha256
                .as_ref()
                .is_none_or(|sha| sha == evidence_manifest_sha256)
                && approval
                    .archived_manifest_sha256
                    .as_ref()
                    .is_none_or(|sha| sha == evidence_manifest_sha256),
            "archived evidence manifest hash mismatch",
        ),
        (
            decision.market_candidate_sha256 == market_candidate_sha256
                && decision.market_candidate_sha256 == approval.market_candidate_sha256,
            "market candidate hash mismatch",
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
        market_candidate_bound: is_sha256(input.market_candidate_sha256)
            && input.approval.market_candidate_sha256 == input.market_candidate_sha256,
        max_order_notional_ok: notional_lte(
            &input.market.notional_usd,
            &input.risk_limits.max_order_notional_usd,
        ),
        max_daily_notional_ok: daily_notional_lte(
            &input.risk_limits.daily_used_notional_usd,
            &input.market.notional_usd,
            &input.risk_limits.max_daily_notional_usd,
        ),
        execution_style_gtc_post_only_cancel: input.approval.execution_style
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
            limit_price: candidate.limit_price.clone(),
            // For the live GTC post-only BUY path this is the share size, not
            // the USDC amount. The risk value is limit_price * size.
            size: candidate.target_size.clone(),
            notional_usd: candidate_notional_usd(candidate).unwrap_or_default(),
            selection_reason:
                "highest liquidity candidate within active/accepting/spread/min-order/post-only/notional-depth constraints"
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
    let mut max_ask_size: Option<ParsedDecimal> = None;
    let mut min_spread_bps: Option<u64> = None;
    let mut min_order_size_blocks = false;

    for candidate in candidates {
        if let Some(ask_size) = parse_decimal(&candidate.ask_size) {
            max_ask_size = Some(max_ask_size.map_or(ask_size.clone(), |current| {
                if ask_size > current {
                    ask_size
                } else {
                    current
                }
            }));
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
        if candidate.side != REAL_FUNDS_CANARY_SIDE {
            rejection_counts.wrong_side += 1;
        }
        if candidate.order_type != REAL_FUNDS_CANARY_ORDER_TYPE {
            rejection_counts.wrong_order_type += 1;
        }
        if !timestamp_is_rfc3339(&candidate.book_snapshot_timestamp) {
            rejection_counts.missing_book_snapshot_timestamp += 1;
        }
        if !human_review_ref_present(&candidate.human_review_ref) {
            rejection_counts.missing_human_review_ref += 1;
        }
        if !decimal_gt_zero(&candidate.target_size) {
            rejection_counts.missing_or_zero_target_size += 1;
        }
        if candidate.spread_bps > MAX_SPREAD_BPS {
            rejection_counts.spread_too_wide += 1;
        }
        if !decimal_gt_zero(&candidate.best_ask) {
            rejection_counts.missing_or_zero_best_ask += 1;
        }
        if !ask_size_gte_target_size(candidate) {
            rejection_counts.insufficient_ask_size += 1;
        }
        if !target_notional_lte(candidate, max_notional_usd) {
            rejection_counts.notional_over_cap += 1;
        }
        if !min_order_size_lte_target_size(candidate) {
            rejection_counts.min_order_size_above_order_size += 1;
            min_order_size_blocks = true;
        }
        let rule_snapshot_valid = exchange_rule_snapshot_valid(candidate);
        if !rule_snapshot_valid {
            rejection_counts.exchange_rule_snapshot_invalid += 1;
        }
        if !post_only_limit_terms_valid(candidate) {
            rejection_counts.post_only_not_bound += 1;
        }
        if market_candidate_is_safe(candidate, max_notional_usd) {
            safe_candidates += 1;
        }
    }

    RealFundsCanaryMarketDiagnostics {
        market_validation_complete: true,
        candidates_seen: candidates.len() as u64,
        safe_candidates,
        max_ask_size: max_ask_size.map(|value| value.format_summary()),
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
        && candidate.side == REAL_FUNDS_CANARY_SIDE
        && candidate.order_type == REAL_FUNDS_CANARY_ORDER_TYPE
        && timestamp_is_rfc3339(&candidate.book_snapshot_timestamp)
        && human_review_ref_present(&candidate.human_review_ref)
        && candidate.spread_bps <= MAX_SPREAD_BPS
        && decimal_gt_zero(&candidate.best_ask)
        && decimal_gt_zero(&candidate.target_size)
        && ask_size_gte_target_size(candidate)
        && target_notional_lte(candidate, max_notional_usd)
        && min_order_size_lte_target_size(candidate)
        && exchange_rule_snapshot_valid(candidate)
        && post_only_limit_terms_valid(candidate)
}

fn valid_approval(approval: &RealFundsCanaryApproval) -> bool {
    !approval.approval_id.trim().is_empty()
        && is_sha256(&approval.approval_hash)
        && approval.scope == REAL_FUNDS_CANARY_SCOPE
        && approval.execution_style == REAL_FUNDS_CANARY_EXECUTION_STYLE
        && approval_not_expired(&approval.expires_at)
        && is_sha256(&approval.artifact_sha256)
        && is_sha256(&approval.evidence_manifest_sha256)
        && approval
            .workspace_manifest_sha256
            .as_ref()
            .is_none_or(|sha| is_sha256(sha))
        && approval
            .archived_manifest_sha256
            .as_ref()
            .is_none_or(|sha| is_sha256(sha) && sha == &approval.evidence_manifest_sha256)
        && is_sha256(&approval.market_candidate_sha256)
        && !approval.operator_identity_ref.trim().is_empty()
        && decimal_gt_zero(&approval.max_order_notional_usd)
        && decimal_gt_zero(&approval.max_daily_notional_usd)
}

fn timestamp_is_rfc3339(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value.trim()).is_ok()
}

fn human_review_ref_present(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && !trimmed.contains("REPLACE_WITH")
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
    parse_decimal(value).is_some_and(|value| value.is_positive())
}

fn decimal_lte(left: &str, right: &str) -> bool {
    match (parse_decimal(left), parse_decimal(right)) {
        (Some(left), Some(right)) => left <= right,
        _ => false,
    }
}

fn decimal_lt(left: &str, right: &str) -> bool {
    match (parse_decimal(left), parse_decimal(right)) {
        (Some(left), Some(right)) => left < right,
        _ => false,
    }
}

fn ask_size_gte_target_size(candidate: &RealFundsCanaryMarketCandidate) -> bool {
    match (
        parse_decimal(&candidate.ask_size),
        parse_decimal(&candidate.target_size),
    ) {
        (Some(ask_size), Some(target_size)) => ask_size >= target_size,
        _ => false,
    }
}

fn min_order_size_lte_target_size(candidate: &RealFundsCanaryMarketCandidate) -> bool {
    match (
        parse_decimal(&candidate.min_order_size),
        parse_decimal(&candidate.target_size),
    ) {
        (Some(min_size), Some(target_size)) => min_size <= target_size,
        _ => false,
    }
}

fn target_notional_lte(candidate: &RealFundsCanaryMarketCandidate, cap: &str) -> bool {
    candidate_notional_usd(candidate).is_some_and(|notional| decimal_lte(&notional, cap))
}

fn exchange_rule_snapshot_valid(candidate: &RealFundsCanaryMarketCandidate) -> bool {
    let snapshot = &candidate.exchange_rule_snapshot;
    snapshot.schema_version == 1
        && snapshot.venue == "polymarket_clob"
        && snapshot.order_mode == REAL_FUNDS_CANARY_ORDER_MODE
        && snapshot.order_type == candidate.order_type
        && snapshot.order_type == REAL_FUNDS_CANARY_ORDER_TYPE
        && snapshot.side == candidate.side
        && snapshot.side == REAL_FUNDS_CANARY_SIDE
        && snapshot.target_size_semantics == REAL_FUNDS_CANARY_TARGET_SIZE_SEMANTICS
        && decimal_gt_zero(&snapshot.min_share_size)
        && decimal_gt_zero(&snapshot.min_tick_size)
        && decimal_lte(&snapshot.min_share_size, &candidate.target_size)
        && timestamp_is_rfc3339(&snapshot.captured_at)
        && approval_not_expired(&snapshot.expires_at)
        && rule_source_present(snapshot)
}

fn rule_source_present(snapshot: &ExchangeRuleSnapshot) -> bool {
    let source = snapshot.source.trim();
    let evidence_ref = snapshot.evidence_ref.trim();
    !source.is_empty()
        && !source.contains("REPLACE_WITH")
        && !evidence_ref.is_empty()
        && !evidence_ref.contains("REPLACE_WITH")
}

fn post_only_limit_terms_valid(candidate: &RealFundsCanaryMarketCandidate) -> bool {
    if candidate.side != REAL_FUNDS_CANARY_SIDE
        || candidate.order_type != REAL_FUNDS_CANARY_ORDER_TYPE
        || !candidate.post_only
    {
        return false;
    }
    decimal_gt_zero(&candidate.limit_price)
        && decimal_lt(&candidate.limit_price, &candidate.best_ask)
        && decimal_is_multiple_of(
            &candidate.limit_price,
            &candidate.exchange_rule_snapshot.min_tick_size,
        )
}

fn candidate_notional_usd(candidate: &RealFundsCanaryMarketCandidate) -> Option<String> {
    match (
        parse_decimal(&candidate.limit_price),
        parse_decimal(&candidate.target_size),
    ) {
        (Some(price), Some(size)) => price.mul(&size).map(|value| value.format_summary()),
        _ => None,
    }
}

fn decimal_is_multiple_of(value: &str, step: &str) -> bool {
    match (parse_decimal(value), parse_decimal(step)) {
        (Some(value), Some(step)) if value.is_positive() && step.is_positive() => {
            let scale = value.scale.max(step.scale);
            let Some(left_factor) = pow10(scale - value.scale) else {
                return false;
            };
            let Some(right_factor) = pow10(scale - step.scale) else {
                return false;
            };
            let Some(left) = value.units.checked_mul(left_factor) else {
                return false;
            };
            let Some(right) = step.units.checked_mul(right_factor) else {
                return false;
            };
            right != 0 && left % right == 0
        }
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
        (Some(used), Some(order), Some(cap)) => used.add(&order).is_some_and(|sum| sum <= cap),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedDecimal {
    units: i128,
    scale: u32,
}

impl ParsedDecimal {
    fn is_positive(&self) -> bool {
        self.units > 0
    }

    fn add(&self, other: &Self) -> Option<Self> {
        let scale = self.scale.max(other.scale);
        let left = self.units.checked_mul(pow10(scale - self.scale)?)?;
        let right = other.units.checked_mul(pow10(scale - other.scale)?)?;
        Some(Self {
            units: left.checked_add(right)?,
            scale,
        })
    }

    fn mul(&self, other: &Self) -> Option<Self> {
        Some(Self {
            units: self.units.checked_mul(other.units)?,
            scale: self.scale.checked_add(other.scale)?,
        })
    }

    fn format_summary(&self) -> String {
        if self.scale == 0 {
            return self.units.to_string();
        }
        let divisor = pow10(self.scale).expect("validated scale must fit pow10");
        let whole = self.units / divisor;
        let fraction = (self.units % divisor).abs();
        let mut fraction_text = format!("{:0width$}", fraction, width = self.scale as usize);
        while fraction_text.ends_with('0') {
            fraction_text.pop();
        }
        if fraction_text.is_empty() {
            whole.to_string()
        } else {
            format!("{whole}.{fraction_text}")
        }
    }
}

impl PartialOrd for ParsedDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let scale = self.scale.max(other.scale);
        let left = self.units.checked_mul(pow10(scale - self.scale)?)?;
        let right = other.units.checked_mul(pow10(scale - other.scale)?)?;
        left.partial_cmp(&right)
    }
}

fn parse_decimal(value: &str) -> Option<ParsedDecimal> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value || trimmed.starts_with('-') {
        return None;
    }
    let (whole, fraction) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !fraction.is_empty() && !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if fraction.len() > 12
        || whole.len() > 18
        || whole.len() + fraction.len() > 24
        || (trimmed.contains('.') && fraction.is_empty())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let units = digits.parse::<i128>().ok()?;
    Some(ParsedDecimal {
        units,
        scale: fraction.len() as u32,
    })
}

fn pow10(scale: u32) -> Option<i128> {
    10_i128.checked_pow(scale)
}
