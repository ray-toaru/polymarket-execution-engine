use super::*;

pub(crate) async fn load_submit_receipt(
    backend: &ServiceBackend,
    execution_id: &str,
) -> Result<SubmitReceipt, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => service.load_submit_receipt(execution_id).await,
        ServiceBackend::Postgres(service) => service.load_submit_receipt(execution_id).await,
    }
}

pub(crate) async fn record_sign_only_lifecycle_event(
    backend: &ServiceBackend,
    record: SignOnlyLifecycleRecord,
) -> Result<SignOnlyLifecycleRecord, ServiceError> {
    match backend {
        ServiceBackend::InMemory(service) => service.record_sign_only_lifecycle_event(record).await,
        ServiceBackend::Postgres(service) => service.record_sign_only_lifecycle_event(record).await,
    }
}
