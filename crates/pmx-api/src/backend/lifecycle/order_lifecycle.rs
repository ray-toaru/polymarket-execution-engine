use super::*;

pub(crate) async fn record_non_live_cancel_request(
    backend: &ServiceBackend,
    order_id: &str,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => {
            service
                .record_non_live_cancel_request(order_id, reason, correlation_id)
                .await
        }
        ServiceBackend::Postgres(service) => {
            service
                .record_non_live_cancel_request(order_id, reason, correlation_id)
                .await
        }
    }
}

pub(crate) async fn reconcile_order_lifecycle_divergence(
    backend: &ServiceBackend,
    order_id: &str,
    account_id: Option<&str>,
    remote_observation: RemoteOrderObservation,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<Option<(OrderLifecycleDivergence, Option<OrderLifecycleRecord>)>, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => {
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
        ServiceBackend::Postgres(service) => {
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

pub(crate) async fn list_order_lifecycle_events(
    backend: &ServiceBackend,
    query: OrderLifecycleEventQuery,
) -> Result<Vec<OrderLifecycleEventRecord>, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => service.list_order_lifecycle_events(query).await,
        ServiceBackend::Postgres(service) => service.list_order_lifecycle_events(query).await,
    }
}
