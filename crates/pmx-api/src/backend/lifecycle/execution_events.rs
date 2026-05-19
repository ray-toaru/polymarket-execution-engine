use super::*;

pub(crate) async fn record_execution_lifecycle_event(
    backend: &ServiceBackend,
    event: ExecutionLifecycleEvent,
) -> Result<(), ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => service.record_execution_lifecycle_event(event).await,
        ServiceBackend::Postgres(service) => service.record_execution_lifecycle_event(event).await,
    }
}

pub(crate) async fn list_execution_lifecycle_events(
    backend: &ServiceBackend,
    query: ExecutionLifecycleQuery,
) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => service.list_execution_lifecycle_events(query).await,
        ServiceBackend::Postgres(service) => service.list_execution_lifecycle_events(query).await,
    }
}
