use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use crate::{ConditionId, CoreError, DecimalString, Side, TokenId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BookLevel {
    pub price: DecimalString,
    pub shares: DecimalString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketDataFreshness {
    Fresh,
    Stale,
    FutureDated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketBookSnapshot {
    pub condition_id: ConditionId,
    pub token_id: TokenId,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub observed_at_ms: i64,
    pub valid_for_ms: i64,
}

impl MarketBookSnapshot {
    pub fn freshness_at(&self, now_ms: i64) -> MarketDataFreshness {
        if now_ms < self.observed_at_ms {
            return MarketDataFreshness::FutureDated;
        }
        if self.valid_for_ms < 0 || now_ms - self.observed_at_ms > self.valid_for_ms {
            return MarketDataFreshness::Stale;
        }
        MarketDataFreshness::Fresh
    }

    pub fn require_fresh_at(&self, now_ms: i64) -> Result<(), CoreError> {
        match self.freshness_at(now_ms) {
            MarketDataFreshness::Fresh => Ok(()),
            MarketDataFreshness::Stale => Err(CoreError::StaleMarketData),
            MarketDataFreshness::FutureDated => Err(CoreError::FutureDatedMarketData),
        }
    }

    pub fn top_level_for(&self, side: &Side) -> Option<&BookLevel> {
        match side {
            Side::Buy => self.asks.first(),
            Side::Sell => self.bids.first(),
        }
    }

    pub fn has_top_liquidity_for(
        &self,
        side: &Side,
        shares: &DecimalString,
        now_ms: i64,
    ) -> Result<bool, CoreError> {
        self.require_fresh_at(now_ms)?;
        shares.validate_positive()?;
        let Some(level) = self.top_level_for(side) else {
            return Ok(false);
        };
        level.price.validate_limit_price()?;
        level.shares.validate_positive()?;
        Ok(compare_decimal(&level.shares, shares)? != Ordering::Less)
    }
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
