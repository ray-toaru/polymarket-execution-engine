use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    LiveReadEventQuery, LiveReadEventRecord, LiveReadEventStore, OrderLifecycleEventRecord,
    OrderLifecycleStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
    RuntimeWorkerStatusStore, SignOnlyLifecycleStore,
};

use super::ExecutorService;
use crate::model::ServiceError;
use crate::runtime_state::RuntimeStateProvider;

impl<S, R> ExecutorService<S, R>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + LiveReadEventStore
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

    pub async fn list_live_read_events(
        &self,
        query: LiveReadEventQuery,
    ) -> Result<Vec<LiveReadEventRecord>, ServiceError> {
        Ok(self.store.list_live_read_events(&query).await?)
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
}
