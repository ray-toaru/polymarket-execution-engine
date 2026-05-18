use pmx_core::{OrderEventKind, OrderLifecycleDivergence, RemoteOrderObservation};
use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleRecord, OrderLifecycleStore, RuntimeWorkerStatusStore, SignOnlyLifecycleStore,
};

use super::ExecutorService;
use crate::model::ServiceError;
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
}
