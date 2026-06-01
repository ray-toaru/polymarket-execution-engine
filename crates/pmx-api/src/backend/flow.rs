use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent, TradeIntent,
};
use pmx_service::{CompilePlanByIdCommand, DecisionByIdRequest, ServiceError};

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn normalize_with_correlation(
        &self,
        intent: TradeIntent,
        correlation_id: Option<String>,
    ) -> Result<NormalizedIntent, ServiceError> {
        match self {
            Self::InMemory(service) => {
                service
                    .normalize_with_correlation(intent, correlation_id.clone())
                    .await
            }
            Self::Postgres(service) => {
                service
                    .normalize_with_correlation(intent, correlation_id)
                    .await
            }
        }
    }

    pub(crate) async fn capture_snapshot_with_correlation(
        &self,
        normalized: NormalizedIntent,
        correlation_id: Option<String>,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        match self {
            Self::InMemory(service) => {
                service
                    .capture_snapshot_with_correlation(normalized, correlation_id.clone())
                    .await
            }
            Self::Postgres(service) => {
                service
                    .capture_snapshot_with_correlation(normalized, correlation_id)
                    .await
            }
        }
    }

    pub(crate) async fn evaluate_decision_by_id(
        &self,
        req: DecisionByIdRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        match self {
            Self::InMemory(service) => service.evaluate_decision_by_id(req).await,
            Self::Postgres(service) => service.evaluate_decision_by_id(req).await,
        }
    }

    pub(crate) async fn compile_plan_by_id(
        &self,
        req: CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        match self {
            Self::InMemory(service) => service.compile_plan_by_id(req).await,
            Self::Postgres(service) => service.compile_plan_by_id(req).await,
        }
    }
}
