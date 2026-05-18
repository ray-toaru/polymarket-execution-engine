use pmx_core::SignOnlyLifecycleRecord;
use pmx_service::{
    ServiceError, StandardSignOnlyConstructionReceipt, StandardSignOnlyConstructionRequest,
};
use pmx_store::SignOnlyLifecycleQuery;

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn record_standard_sign_only_construction(
        &self,
        req: StandardSignOnlyConstructionRequest,
    ) -> Result<StandardSignOnlyConstructionReceipt, ServiceError> {
        match self {
            Self::InMemory(service) => service.record_standard_sign_only_construction(req).await,
            Self::Postgres(service) => service.record_standard_sign_only_construction(req).await,
        }
    }

    pub(crate) async fn list_sign_only_lifecycle_events(
        &self,
        query: SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_sign_only_lifecycle_events(query).await,
            Self::Postgres(service) => service.list_sign_only_lifecycle_events(query).await,
        }
    }
}
