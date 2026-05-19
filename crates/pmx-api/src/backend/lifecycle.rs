use pmx_core::{
    OrderLifecycleDivergence, RemoteOrderObservation, SignOnlyLifecycleRecord, SubmitReceipt,
};
use pmx_service::ServiceError;
use pmx_store::{
    ExecutionLifecycleEvent, ExecutionLifecycleQuery, OrderLifecycleEventQuery,
    OrderLifecycleEventRecord, OrderLifecycleRecord,
};

use super::ServiceBackend;

#[path = "lifecycle/execution_events.rs"]
mod execution_events;

#[path = "lifecycle/order_lifecycle.rs"]
mod order_lifecycle;

#[path = "lifecycle/sign_only_receipt.rs"]
mod sign_only_receipt;

impl ServiceBackend {
    pub(crate) async fn record_execution_lifecycle_event(
        &self,
        event: ExecutionLifecycleEvent,
    ) -> Result<(), ServiceError> {
        execution_events::record_execution_lifecycle_event(self, event).await
    }

    pub(crate) async fn list_execution_lifecycle_events(
        &self,
        query: ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
        execution_events::list_execution_lifecycle_events(self, query).await
    }

    pub(crate) async fn record_non_live_cancel_request(
        &self,
        order_id: &str,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        order_lifecycle::record_non_live_cancel_request(self, order_id, reason, correlation_id)
            .await
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
        order_lifecycle::reconcile_order_lifecycle_divergence(
            self,
            order_id,
            account_id,
            remote_observation,
            reason,
            correlation_id,
        )
        .await
    }

    pub(crate) async fn list_order_lifecycle_events(
        &self,
        query: OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, ServiceError> {
        order_lifecycle::list_order_lifecycle_events(self, query).await
    }

    pub(crate) async fn load_submit_receipt(
        &self,
        execution_id: &str,
    ) -> Result<SubmitReceipt, ServiceError> {
        sign_only_receipt::load_submit_receipt(self, execution_id).await
    }

    pub(crate) async fn record_sign_only_lifecycle_event(
        &self,
        record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        sign_only_receipt::record_sign_only_lifecycle_event(self, record).await
    }
}
