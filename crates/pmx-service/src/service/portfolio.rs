use pmx_core::{AccountId, PortfolioProjection, RiskDecision, RiskLimits, assess_exposure};
use pmx_store::PortfolioProjectionStore;

use super::ExecutorService;
use crate::model::ServiceError;
use crate::runtime_state::RuntimeStateProvider;

impl<S, R> ExecutorService<S, R>
where
    S: PortfolioProjectionStore + Clone + Send + Sync + 'static,
    R: RuntimeStateProvider,
{
    pub async fn record_portfolio_projection(
        &self,
        projection: PortfolioProjection,
    ) -> Result<(), ServiceError> {
        self.store.save_portfolio_projection(&projection).await?;
        Ok(())
    }

    pub async fn load_portfolio_projection(
        &self,
        account_id: &AccountId,
    ) -> Result<PortfolioProjection, ServiceError> {
        Ok(self.store.load_portfolio_projection(account_id).await?)
    }

    pub async fn assess_portfolio_risk(
        &self,
        account_id: &AccountId,
        limits: RiskLimits,
    ) -> Result<RiskDecision, ServiceError> {
        let projection = self.store.load_portfolio_projection(account_id).await?;
        assess_exposure(&projection.exposure, &limits)
            .map_err(|err| ServiceError::BadRequest(err.to_string()))
    }
}
