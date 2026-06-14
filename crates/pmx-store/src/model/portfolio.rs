use async_trait::async_trait;
use pmx_core::{AccountId, PortfolioProjection};

use super::StoreError;

#[async_trait]
pub trait PortfolioProjectionStore: Send + Sync {
    async fn save_portfolio_projection(
        &self,
        projection: &PortfolioProjection,
    ) -> Result<(), StoreError>;

    async fn load_portfolio_projection(
        &self,
        account_id: &AccountId,
    ) -> Result<PortfolioProjection, StoreError>;
}
