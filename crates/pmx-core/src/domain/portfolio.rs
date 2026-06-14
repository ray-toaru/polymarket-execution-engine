use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{AccountId, CoreError, DecimalString, InternalOrderId, Side, TokenId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillRecord {
    pub fill_id: String,
    pub order_id: InternalOrderId,
    pub token_id: TokenId,
    pub side: Side,
    pub price: DecimalString,
    pub shares: DecimalString,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionProjection {
    pub token_id: TokenId,
    pub shares: DecimalString,
    pub average_price: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenOrderProjection {
    pub order_id: InternalOrderId,
    pub token_id: TokenId,
    pub side: Side,
    pub remaining_shares: DecimalString,
    pub limit_price: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureProjection {
    pub gross_notional: DecimalString,
    pub open_order_notional: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioProjection {
    pub account_id: AccountId,
    pub fills: Vec<FillRecord>,
    pub positions: Vec<PositionProjection>,
    pub open_orders: Vec<OpenOrderProjection>,
    pub exposure: ExposureProjection,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskLimits {
    pub max_gross_notional: DecimalString,
    pub max_open_order_notional: DecimalString,
    pub kill_switch_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskBlockReason {
    KillSwitchActive,
    GrossExposureExceeded,
    OpenOrderExposureExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "decision",
    content = "reason",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum RiskDecision {
    Allow,
    Block(RiskBlockReason),
}

pub fn assess_exposure(
    exposure: &ExposureProjection,
    limits: &RiskLimits,
) -> Result<RiskDecision, CoreError> {
    if limits.kill_switch_active {
        return Ok(RiskDecision::Block(RiskBlockReason::KillSwitchActive));
    }
    if compare_decimal(&exposure.gross_notional, &limits.max_gross_notional)? == Ordering::Greater {
        return Ok(RiskDecision::Block(RiskBlockReason::GrossExposureExceeded));
    }
    if compare_decimal(
        &exposure.open_order_notional,
        &limits.max_open_order_notional,
    )? == Ordering::Greater
    {
        return Ok(RiskDecision::Block(
            RiskBlockReason::OpenOrderExposureExceeded,
        ));
    }
    Ok(RiskDecision::Allow)
}

fn compare_decimal(left: &DecimalString, right: &DecimalString) -> Result<Ordering, CoreError> {
    left.validate()?;
    right.validate()?;
    let (left_int, left_frac) = split_decimal(&left.0);
    let (right_int, right_frac) = split_decimal(&right.0);
    Ok(left_int
        .len()
        .cmp(&right_int.len())
        .then_with(|| left_int.cmp(right_int))
        .then_with(|| compare_fraction(left_frac, right_frac)))
}

fn split_decimal(value: &str) -> (&str, &str) {
    value.split_once('.').unwrap_or((value, ""))
}

fn compare_fraction(left: &str, right: &str) -> Ordering {
    let width = left.len().max(right.len());
    left.bytes()
        .chain(std::iter::repeat(b'0'))
        .take(width)
        .cmp(right.bytes().chain(std::iter::repeat(b'0')).take(width))
}
