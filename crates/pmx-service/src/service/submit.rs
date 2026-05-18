use pmx_core::SubmitReceipt;
use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleStore, RuntimeWorkerStatusStore, SignOnlyLifecycleStore,
};

use super::ExecutorService;
use crate::model::{ServiceError, SubmitOutcome, SubmitPlanCommand};
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
