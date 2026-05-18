use pmx_core::SignOnlyLifecycleRecord;
use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleStore, RuntimeWorkerStatusStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore,
};

use super::ExecutorService;
use crate::model::{
    ServiceError, StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest,
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
}
