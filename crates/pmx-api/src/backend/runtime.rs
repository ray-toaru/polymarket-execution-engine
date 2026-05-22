use pmx_service::ServiceError;
use pmx_store::{
    KillSwitchStateChange, RuntimeControlStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
};

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

    pub(crate) async fn set_account_kill_switch(
        &self,
        account_id: &pmx_core::AccountId,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, ServiceError> {
        match self {
            Self::InMemory(service) => service
                .store()
                .set_account_kill_switch(account_id, enabled, reason)
                .await
                .map_err(ServiceError::from),
            Self::Postgres(service) => service
                .store()
                .set_account_kill_switch(account_id, enabled, reason)
                .await
                .map_err(ServiceError::from),
        }
    }

    pub(crate) async fn set_global_kill_switch(
        &self,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, ServiceError> {
        match self {
            Self::InMemory(service) => service
                .store()
                .set_global_kill_switch(enabled, reason)
                .await
                .map_err(ServiceError::from),
            Self::Postgres(service) => service
                .store()
                .set_global_kill_switch(enabled, reason)
                .await
                .map_err(ServiceError::from),
        }
    }
}
