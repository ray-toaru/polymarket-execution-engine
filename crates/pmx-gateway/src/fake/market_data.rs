use async_trait::async_trait;
use pmx_core::{ConditionId, MarketBookSnapshot, TokenId};

use crate::{GatewayError, MarketDataReader};

use super::FakeGateway;

#[async_trait]
impl MarketDataReader for FakeGateway {
    async fn read_market_book(
        &self,
        condition_id: &ConditionId,
        token_id: &TokenId,
    ) -> Result<MarketBookSnapshot, GatewayError> {
        let lock = self.inner.lock().expect("fake gateway mutex poisoned");
        lock.read_failure.apply()?;
        lock.market_books
            .get(&(condition_id.clone(), token_id.clone()))
            .cloned()
            .ok_or_else(|| GatewayError::RemoteUnknown("market book snapshot not found".into()))
    }
}
