use super::*;
use serde::Serialize;

fn base_intent(side: Side, quantity: QuantityIntent) -> TradeIntent {
    TradeIntent {
        client_intent_id: "intent-1".into(),
        account_id: AccountId("acct-1".into()),
        market: MarketRef {
            condition_id: ConditionId("cond-1".into()),
            slug: None,
            is_sports: false,
        },
        token_id: TokenId("token-1".into()),
        side,
        quantity,
        limit_price: DecimalString("0.51".into()),
        time_in_force: TimeInForce::Gtc,
        collateral_profile_id: None,
    }
}

#[path = "domain_tests/intent_normalization.rs"]
mod intent_normalization;

#[path = "domain_tests/lifecycle.rs"]
mod lifecycle;

#[path = "domain_tests/divergence.rs"]
mod divergence;

#[path = "domain_tests/hash_value.rs"]
mod hash_value;

#[path = "domain_tests/non_live_foundation.rs"]
mod non_live_foundation;
