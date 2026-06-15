use async_trait::async_trait;
use pmx_core::{AccountId, PortfolioProjection};

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{PortfolioProjectionStore, StoreError};

#[async_trait]
impl PortfolioProjectionStore for PostgresStore {
    async fn save_portfolio_projection(
        &self,
        projection: &PortfolioProjection,
    ) -> Result<(), StoreError> {
        let payload = serde_json::to_value(projection)
            .map_err(|err| StoreError::InvalidData(err.to_string()))?;
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO portfolio_projections \
                 (account_id, projection_json, observed_at_ms) VALUES ($1, $2, $3) \
                 ON CONFLICT (account_id) DO UPDATE SET \
                 projection_json = EXCLUDED.projection_json, \
                 observed_at_ms = EXCLUDED.observed_at_ms, updated_at = now() \
                 WHERE portfolio_projections.observed_at_ms < EXCLUDED.observed_at_ms",
                &[
                    &projection.account_id.0,
                    &payload,
                    &projection.observed_at_ms,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_portfolio_projection(
        &self,
        account_id: &AccountId,
    ) -> Result<PortfolioProjection, StoreError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT projection_json FROM portfolio_projections WHERE account_id = $1",
                &[&account_id.0],
            )
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| StoreError::NotFound(format!("portfolio account={}", account_id.0)))?;
        serde_json::from_value(row.get(0)).map_err(|err| StoreError::InvalidData(err.to_string()))
    }
}
