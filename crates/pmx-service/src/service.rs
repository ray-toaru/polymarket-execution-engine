use pmx_core::*;
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusReport, RuntimeWorkerStatusStore, SignOnlyLifecycleQuery,
    SignOnlyLifecycleStore,
};

use crate::model::*;
use crate::runtime_state::{FailClosedRuntimeStateProvider, RuntimeStateProvider};

#[derive(Debug, Clone)]
pub struct ExecutorService<S, R = FailClosedRuntimeStateProvider> {
    store: S,
    runtime_state_provider: R,
    executor_version: String,
    contract_version: String,
}

impl<S> ExecutorService<S, FailClosedRuntimeStateProvider>
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
{
    pub fn new(store: S) -> Self {
        Self::with_runtime_provider(
            store,
            FailClosedRuntimeStateProvider,
            env!("CARGO_PKG_VERSION").to_owned(),
            DEFAULT_CONTRACT_VERSION.to_owned(),
        )
    }
}

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
    pub fn with_runtime_provider(
        store: S,
        runtime_state_provider: R,
        executor_version: String,
        contract_version: String,
    ) -> Self {
        Self {
            store,
            runtime_state_provider,
            executor_version,
            contract_version,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn record_admin_audit_event(
        &self,
        event: AdminAuditEvent,
    ) -> Result<(), ServiceError> {
        self.store.record_admin_audit_event(&event).await?;
        Ok(())
    }

    pub async fn list_admin_audit_events(
        &self,
        query: AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, ServiceError> {
        Ok(self.store.list_admin_audit_events(&query).await?)
    }

    pub async fn record_execution_lifecycle_event(
        &self,
        event: ExecutionLifecycleEvent,
    ) -> Result<(), ServiceError> {
        self.store.record_execution_lifecycle_event(&event).await?;
        Ok(())
    }

    pub async fn list_execution_lifecycle_events(
        &self,
        query: ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
        Ok(self.store.list_execution_lifecycle_events(&query).await?)
    }

    pub async fn list_order_lifecycle_events(
        &self,
        query: pmx_store::OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, ServiceError> {
        Ok(self.store.list_order_lifecycle_events(&query).await?)
    }

    pub async fn list_runtime_worker_status(
        &self,
        query: RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, ServiceError> {
        if query.account_id.trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "account_id must be non-empty".into(),
            ));
        }
        Ok(self.store.list_runtime_worker_status(&query).await?)
    }

    pub async fn record_non_live_cancel_request(
        &self,
        order_id: &str,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        crate::order_lifecycle::record_non_live_cancel_request(
            &self.store,
            order_id,
            reason,
            correlation_id,
        )
        .await
    }

    pub async fn record_non_live_reconcile_observation(
        &self,
        order_id: &str,
        event: OrderEventKind,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        crate::order_lifecycle::record_non_live_reconcile_observation(
            &self.store,
            order_id,
            event,
            reason,
            correlation_id,
        )
        .await
    }

    pub async fn reconcile_order_lifecycle_divergence(
        &self,
        order_id: &str,
        account_id: Option<&str>,
        remote_observation: RemoteOrderObservation,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<(OrderLifecycleDivergence, Option<OrderLifecycleRecord>)>, ServiceError>
    {
        crate::order_lifecycle::reconcile_order_lifecycle_divergence(
            &self.store,
            order_id,
            account_id,
            remote_observation,
            reason,
            correlation_id,
        )
        .await
    }

    pub async fn record_sign_only_lifecycle_event(
        &self,
        record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        crate::sign_only::record_sign_only_lifecycle_event(&self.store, record).await
    }

    pub async fn list_sign_only_lifecycle_events(
        &self,
        query: SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, ServiceError> {
        Ok(self.store.list_sign_only_lifecycle_events(&query).await?)
    }

    pub async fn record_standard_sign_only_construction(
        &self,
        req: StandardSignOnlyConstructionRequest,
    ) -> Result<StandardSignOnlyConstructionReceipt, ServiceError> {
        crate::sign_only::record_standard_sign_only_construction(&self.store, req).await
    }

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

    pub async fn submit_plan(&self, req: SubmitPlanCommand) -> Result<SubmitOutcome, ServiceError> {
        crate::submit::submit_plan(
            &self.store,
            req,
            &self.executor_version,
            &self.contract_version,
        )
        .await
    }

    pub async fn load_submit_receipt(
        &self,
        execution_id: &str,
    ) -> Result<SubmitReceipt, ServiceError> {
        Ok(self.store.load_submit_receipt(execution_id).await?)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
