use pmx_core::SubmitReceipt;
use pmx_gateway::{ClobGateway, SignerProvider};
use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleStore, OrderReconcileBacklogStore, RuntimeWorkerStatusStore,
    SignOnlyLifecycleStore,
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
        + OrderReconcileBacklogStore
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

    pub async fn submit_plan_with_gateway<P, G>(
        &self,
        req: SubmitPlanCommand,
        signer_provider: &P,
        gateway: &G,
    ) -> Result<SubmitOutcome, ServiceError>
    where
        P: SignerProvider,
        G: ClobGateway,
    {
        crate::submit::submit_plan_with_gateway(
            &self.store,
            &self.runtime_state_provider,
            signer_provider,
            gateway,
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
