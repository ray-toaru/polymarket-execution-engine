use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent, TradeIntent,
};
use pmx_gateway::MarketDataReader;
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
        crate::plan_flow::normalize(&self.store, intent, None).await
    }

    pub async fn normalize_with_correlation(
        &self,
        intent: TradeIntent,
        correlation_id: Option<String>,
    ) -> Result<NormalizedIntent, ServiceError> {
        crate::plan_flow::normalize(&self.store, intent, correlation_id).await
    }

    pub async fn capture_snapshot(
        &self,
        normalized: NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        crate::plan_flow::capture_snapshot(
            &self.store,
            &self.runtime_state_provider,
            normalized,
            None,
        )
        .await
    }

    pub async fn capture_snapshot_with_correlation(
        &self,
        normalized: NormalizedIntent,
        correlation_id: Option<String>,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        crate::plan_flow::capture_snapshot(
            &self.store,
            &self.runtime_state_provider,
            normalized,
            correlation_id,
        )
        .await
    }

    pub async fn capture_snapshot_with_market_data<M>(
        &self,
        normalized: NormalizedIntent,
        market_data_reader: &M,
        now_ms: i64,
    ) -> Result<FeasibilitySnapshot, ServiceError>
    where
        M: MarketDataReader,
    {
        crate::plan_flow::capture_snapshot_with_market_data(
            &self.store,
            &self.runtime_state_provider,
            market_data_reader,
            normalized,
            now_ms,
            None,
        )
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
    /// This is the preferred public API path from v0.14 onward: the executor adapter supplies
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
        crate::plan_flow::compile_plan(
            &self.store,
            req,
            &self.executor_version,
            &self.contract_version,
        )
        .await
    }

    /// Compile a plan by loading all prior objects from the executor store.
    ///
    /// This prevents client-side object graph splicing such as Intent A + Snapshot B + Decision C.
    pub async fn compile_plan_by_id(
        &self,
        req: CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        crate::plan_flow::compile_plan_by_id(
            &self.store,
            req,
            &self.executor_version,
            &self.contract_version,
        )
        .await
    }
}
