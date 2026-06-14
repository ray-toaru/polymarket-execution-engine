use serde::{Deserialize, Serialize};

use crate::{ConditionId, CoreError, DecimalString, TokenId};

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
}
