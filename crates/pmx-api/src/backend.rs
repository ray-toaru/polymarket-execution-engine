use pmx_core::*;
use pmx_service::{
    ExecutorService, ServiceError, StandardSignOnlyConstructionReceipt,
    StandardSignOnlyConstructionRequest, StoreBackedRuntimeStateProvider, SubmitOutcome,
};
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, ExecutionLifecycleEvent, ExecutionLifecycleQuery,
    InMemoryStore, OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord,
    PostgresStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport, SignOnlyLifecycleQuery,
};

pub(crate) const CONTRACT_VERSION: &str = "1.0.0-draft";

#[derive(Clone)]
pub enum ServiceBackend {
    InMemory(ExecutorService<InMemoryStore>),
    Postgres(ExecutorService<PostgresStore, StoreBackedRuntimeStateProvider<PostgresStore>>),
}

impl ServiceBackend {
    pub(crate) fn storage_mode(&self) -> &'static str {
        match self {
            Self::InMemory(_) => "in_memory_scaffold",
            Self::Postgres(_) => "postgres",
        }
    }

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
        req: pmx_service::DecisionByIdRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        match self {
            Self::InMemory(service) => service.evaluate_decision_by_id(req).await,
            Self::Postgres(service) => service.evaluate_decision_by_id(req).await,
        }
    }

    pub(crate) async fn compile_plan_by_id(
        &self,
        req: pmx_service::CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        match self {
            Self::InMemory(service) => service.compile_plan_by_id(req).await,
            Self::Postgres(service) => service.compile_plan_by_id(req).await,
        }
    }

    pub(crate) async fn submit_plan(
        &self,
        req: pmx_service::SubmitPlanCommand,
    ) -> Result<SubmitOutcome, ServiceError> {
        match self {
            Self::InMemory(service) => service.submit_plan(req).await,
            Self::Postgres(service) => service.submit_plan(req).await,
        }
    }

    pub(crate) async fn record_admin_audit_event(
        &self,
        event: AdminAuditEvent,
    ) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_admin_audit_event(event).await,
            Self::Postgres(service) => service.record_admin_audit_event(event).await,
        }
    }

    pub(crate) async fn list_admin_audit_events(
        &self,
        query: AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_admin_audit_events(query).await,
            Self::Postgres(service) => service.list_admin_audit_events(query).await,
        }
    }

    pub(crate) async fn record_execution_lifecycle_event(
        &self,
        event: ExecutionLifecycleEvent,
    ) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_execution_lifecycle_event(event).await,
            Self::Postgres(service) => service.record_execution_lifecycle_event(event).await,
        }
    }

    pub(crate) async fn record_non_live_cancel_request(
        &self,
        order_id: &str,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => {
                service
                    .record_non_live_cancel_request(order_id, reason, correlation_id)
                    .await
            }
            Self::Postgres(service) => {
                service
                    .record_non_live_cancel_request(order_id, reason, correlation_id)
                    .await
            }
        }
    }

    pub(crate) async fn reconcile_order_lifecycle_divergence(
        &self,
        order_id: &str,
        account_id: Option<&str>,
        remote_observation: RemoteOrderObservation,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<(OrderLifecycleDivergence, Option<OrderLifecycleRecord>)>, ServiceError>
    {
        match self {
            Self::InMemory(service) => {
                service
                    .reconcile_order_lifecycle_divergence(
                        order_id,
                        account_id,
                        remote_observation,
                        reason,
                        correlation_id,
                    )
                    .await
            }
            Self::Postgres(service) => {
                service
                    .reconcile_order_lifecycle_divergence(
                        order_id,
                        account_id,
                        remote_observation,
                        reason,
                        correlation_id,
                    )
                    .await
            }
        }
    }

    pub(crate) async fn list_execution_lifecycle_events(
        &self,
        query: ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_execution_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_execution_lifecycle_events(query).await,
        }
    }

    pub(crate) async fn list_order_lifecycle_events(
        &self,
        query: OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_order_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_order_lifecycle_events(query).await,
        }
    }

    pub(crate) async fn list_runtime_worker_status(
        &self,
        query: RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_runtime_worker_status(query).await,
            Self::Postgres(service) => service.list_runtime_worker_status(query).await,
        }
    }

    pub(crate) async fn record_sign_only_lifecycle_event(
        &self,
        record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        match self {
            Self::InMemory(service) => service.record_sign_only_lifecycle_event(record).await,
            Self::Postgres(service) => service.record_sign_only_lifecycle_event(record).await,
        }
    }

    pub(crate) async fn record_standard_sign_only_construction(
        &self,
        req: StandardSignOnlyConstructionRequest,
    ) -> Result<StandardSignOnlyConstructionReceipt, ServiceError> {
        match self {
            Self::InMemory(service) => service.record_standard_sign_only_construction(req).await,
            Self::Postgres(service) => service.record_standard_sign_only_construction(req).await,
        }
    }

    pub(crate) async fn list_sign_only_lifecycle_events(
        &self,
        query: SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_sign_only_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_sign_only_lifecycle_events(query).await,
        }
    }

    pub(crate) async fn load_submit_receipt(
        &self,
        execution_id: &str,
    ) -> Result<SubmitReceipt, ServiceError> {
        match self {
            Self::InMemory(service) => service.load_submit_receipt(execution_id).await,
            Self::Postgres(service) => service.load_submit_receipt(execution_id).await,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) service: ServiceBackend,
}

impl AppState {
    pub fn in_memory() -> Self {
        Self {
            service: ServiceBackend::InMemory(ExecutorService::new(InMemoryStore::default())),
        }
    }

    pub fn postgres(store: PostgresStore) -> Self {
        let provider = StoreBackedRuntimeStateProvider::new(store.clone());
        Self {
            service: ServiceBackend::Postgres(ExecutorService::with_runtime_provider(
                store,
                provider,
                env!("CARGO_PKG_VERSION").to_owned(),
                CONTRACT_VERSION.to_owned(),
            )),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::in_memory()
    }
}
