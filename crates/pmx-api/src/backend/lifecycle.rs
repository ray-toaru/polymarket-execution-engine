use pmx_core::{
    OrderLifecycleDivergence, RemoteOrderObservation, SignOnlyLifecycleRecord, SubmitReceipt,
};
use pmx_service::ServiceError;
use pmx_store::{
    ExecutionLifecycleEvent, ExecutionLifecycleQuery, OrderLifecycleEventQuery,
    OrderLifecycleEventRecord, OrderLifecycleRecord,
};

use super::ServiceBackend;

impl ServiceBackend {
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

    pub(crate) async fn load_submit_receipt(
        &self,
        execution_id: &str,
    ) -> Result<SubmitReceipt, ServiceError> {
        match self {
            Self::InMemory(service) => service.load_submit_receipt(execution_id).await,
            Self::Postgres(service) => service.load_submit_receipt(execution_id).await,
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
}
