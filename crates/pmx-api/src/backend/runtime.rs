use pmx_service::ServiceError;
use pmx_store::{RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport};

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn list_runtime_worker_status(
        &self,
        query: RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_runtime_worker_status(query).await,
            Self::Postgres(service) => service.list_runtime_worker_status(query).await,
        }
    }
}
