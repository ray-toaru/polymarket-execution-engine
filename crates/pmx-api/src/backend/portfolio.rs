use pmx_core::{AccountId, PortfolioProjection, RiskDecision, RiskLimits};
use pmx_service::ServiceError;

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn record_portfolio_projection(
        &self,
        projection: PortfolioProjection,
    ) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_portfolio_projection(projection).await,
            Self::Postgres(service) => service.record_portfolio_projection(projection).await,
        }
    }

    pub(crate) async fn load_portfolio_projection(
        &self,
        account_id: &AccountId,
    ) -> Result<PortfolioProjection, ServiceError> {
        match self {
            Self::InMemory(service) => service.load_portfolio_projection(account_id).await,
            Self::Postgres(service) => service.load_portfolio_projection(account_id).await,
        }
    }

    pub(crate) async fn assess_portfolio_risk(
        &self,
        account_id: &AccountId,
        limits: RiskLimits,
    ) -> Result<RiskDecision, ServiceError> {
        match self {
            Self::InMemory(service) => service.assess_portfolio_risk(account_id, limits).await,
            Self::Postgres(service) => service.assess_portfolio_risk(account_id, limits).await,
        }
    }
}
