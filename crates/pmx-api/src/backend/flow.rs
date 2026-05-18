use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent, TradeIntent,
};
use pmx_service::{CompilePlanByIdCommand, DecisionByIdRequest, ServiceError};

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn normalize(
        &self,
        intent: TradeIntent,
    ) -> Result<NormalizedIntent, ServiceError> {
        match self {
            Self::InMemory(service) => service.normalize(intent).await,
            Self::Postgres(service) => service.normalize(intent).await,
        }
    }

    pub(crate) async fn capture_snapshot(
        &self,
        normalized: NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        match self {
            Self::InMemory(service) => service.capture_snapshot(normalized).await,
            Self::Postgres(service) => service.capture_snapshot(normalized).await,
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
