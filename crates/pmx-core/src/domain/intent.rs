use serde::{Deserialize, Serialize};

use crate::{
    AccountId, ConditionId, CoreError, DecimalString, HashValue, TokenId, canonical_json_sha256,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Fok,
    Gtd,
    Fak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketRef {
    pub condition_id: ConditionId,
    pub slug: Option<String>,
    pub is_sports: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantityIntent {
    pub max_notional: Option<DecimalString>,
    pub max_shares: Option<DecimalString>,
}

impl QuantityIntent {
    pub fn canonicalize(&self, side: &Side) -> Result<QuantityBound, CoreError> {
        let provided = self.max_notional.is_some() as u8 + self.max_shares.is_some() as u8;
        if provided != 1 {
            return Err(CoreError::QuantityBoundCardinality);
        }
        if let Some(v) = &self.max_notional {
            v.validate_positive()?;
        }
        if let Some(v) = &self.max_shares {
            v.validate_positive()?;
        }
        match (side, &self.max_notional, &self.max_shares) {
            (Side::Buy, Some(v), None) => Ok(QuantityBound::WorstCaseQuoteNotional(v.clone())),
            (Side::Sell, None, Some(v)) => Ok(QuantityBound::WorstCaseBaseShares(v.clone())),
            (Side::Buy, None, Some(v)) => Ok(QuantityBound::Unsupported(format!(
                "BUY max_shares requires an explicit quote conversion rule: {}",
                v.0
            ))),
            (Side::Sell, Some(v), None) => Ok(QuantityBound::Unsupported(format!(
                "SELL max_notional requires an explicit base conversion rule: {}",
                v.0
            ))),
            _ => Err(CoreError::QuantityBoundCardinality),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "amount", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuantityBound {
    WorstCaseQuoteNotional(DecimalString),
    WorstCaseBaseShares(DecimalString),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeIntent {
    pub client_intent_id: String,
    pub account_id: AccountId,
    pub market: MarketRef,
    pub token_id: TokenId,
    pub side: Side,
    pub quantity: QuantityIntent,
    pub limit_price: DecimalString,
    pub time_in_force: TimeInForce,
    pub collateral_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedIntent {
    pub normalized_intent_id: String,
    pub intent_hash: HashValue,
    pub account_id: AccountId,
    pub market: MarketRef,
    pub token_id: TokenId,
    pub side: Side,
    pub quantity_bound: QuantityBound,
    pub limit_price: DecimalString,
    pub time_in_force: TimeInForce,
    pub collateral_profile_id: Option<String>,
}

pub fn normalize_intent(intent: TradeIntent) -> Result<NormalizedIntent, CoreError> {
    intent.limit_price.validate_limit_price()?;
    let quantity_bound = intent.quantity.canonicalize(&intent.side)?;
    let intent_hash = canonical_json_sha256(&intent)?;
    let normalized_intent_id = format!("norm-{}", intent_hash.0);
    Ok(NormalizedIntent {
        normalized_intent_id,
        intent_hash,
        account_id: intent.account_id,
        market: intent.market,
        token_id: intent.token_id,
        side: intent.side,
        quantity_bound,
        limit_price: intent.limit_price,
        time_in_force: intent.time_in_force,
        collateral_profile_id: intent.collateral_profile_id,
    })
}
