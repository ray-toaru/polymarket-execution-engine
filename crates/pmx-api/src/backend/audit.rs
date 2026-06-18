use pmx_service::ServiceError;
use pmx_store::{AdminAuditEvent, AdminAuditQuery, LiveReadEventQuery, LiveReadEventRecord};

use super::ServiceBackend;

impl ServiceBackend {
    pub(crate) async fn record_admin_audit_event(
        &self,
        event: AdminAuditEvent,
    ) -> Result<(), ServiceError> {
        match self {
            Self::InMemory(service) => service.record_admin_audit_event(event).await,
            Self::Postgres(service) => service.record_admin_audit_event(event).await,
        }
    }

    pub(crate) async fn list_admin_audit_events(
        &self,
        query: AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_admin_audit_events(query).await,
            Self::Postgres(service) => service.list_admin_audit_events(query).await,
        }
    }

    pub(crate) async fn list_live_read_events(
        &self,
        query: LiveReadEventQuery,
    ) -> Result<Vec<LiveReadEventRecord>, ServiceError> {
        match self {
            Self::InMemory(service) => service.list_live_read_events(query).await,
            Self::Postgres(service) => service.list_live_read_events(query).await,
        }
    }
}
