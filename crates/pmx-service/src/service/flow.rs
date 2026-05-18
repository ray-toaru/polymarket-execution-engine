use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent, TradeIntent,
};
use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleStore, RuntimeWorkerStatusStore, SignOnlyLifecycleStore,
};

use super::ExecutorService;
use crate::model::{
    CompilePlanByIdCommand, CompilePlanCommand, DecisionByIdRequest, DecisionRequest, ServiceError,
};
use crate::runtime_state::RuntimeStateProvider;

impl<S, R> ExecutorService<S, R>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + RuntimeWorkerStatusStore
        + SignOnlyLifecycleStore
        + Clone
        + Send
        + Sync
        + 'static,
    R: RuntimeStateProvider,
{
    pub async fn normalize(&self, intent: TradeIntent) -> Result<NormalizedIntent, ServiceError> {
        crate::plan_flow::normalize(&self.store, intent).await
    }

    pub async fn capture_snapshot(
        &self,
        normalized: NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        crate::plan_flow::capture_snapshot(&self.store, &self.runtime_state_provider, normalized)
            .await
    }

    pub async fn evaluate_decision(
        &self,
        req: DecisionRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        crate::plan_flow::evaluate_decision(&self.store, req).await
    }

    /// Evaluate constraints by loading the object graph from the executor store.
    ///
    /// This is the preferred public API path from v0.14 onward: the control plane supplies
    /// only server-issued IDs, and the executor validates object ownership before computing
    /// the decision. Full-object methods remain available for internal tests and migration-free
    /// development but must not be used for live funds paths.
    pub async fn evaluate_decision_by_id(
        &self,
        req: DecisionByIdRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        crate::plan_flow::evaluate_decision_by_id(&self.store, req).await
    }

    pub async fn compile_plan(
        &self,
        req: CompilePlanCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        crate::plan_flow::compile_plan(&self.store, req).await
    }

    /// Compile a plan by loading all prior objects from the executor store.
    ///
    /// This prevents client-side object graph splicing such as Intent A + Snapshot B + Decision C.
    pub async fn compile_plan_by_id(
        &self,
        req: CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        crate::plan_flow::compile_plan_by_id(&self.store, req).await
    }
}
