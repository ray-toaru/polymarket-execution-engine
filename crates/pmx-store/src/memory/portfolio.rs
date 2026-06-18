use async_trait::async_trait;
use pmx_core::{AccountId, PortfolioProjection};

use crate::{InMemoryStore, PortfolioProjectionStore, StoreError};

#[async_trait]
impl PortfolioProjectionStore for InMemoryStore {
    async fn save_portfolio_projection(
        &self,
        projection: &PortfolioProjection,
    ) -> Result<(), StoreError> {
        let mut inner = self.inner.lock().expect("in-memory store poisoned");
        let should_update = inner
            .portfolio_projections
            .get(&projection.account_id.0)
            .is_none_or(|existing| existing.observed_at_ms < projection.observed_at_ms);
        if should_update {
            inner
                .portfolio_projections
                .insert(projection.account_id.0.clone(), projection.clone());
        }
        Ok(())
    }

    async fn load_portfolio_projection(
        &self,
        account_id: &AccountId,
    ) -> Result<PortfolioProjection, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store poisoned")
            .portfolio_projections
            .get(&account_id.0)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("portfolio account={}", account_id.0)))
    }
}
