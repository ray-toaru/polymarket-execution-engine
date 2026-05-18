use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminAuditEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<i64>,
    pub principal_subject: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAuditQuery {
    pub limit: usize,
    pub before_audit_id: Option<i64>,
    pub operation: Option<String>,
    pub principal_subject: Option<String>,
    pub result: Option<String>,
    pub correlation_id: Option<String>,
}

impl AdminAuditQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

impl Default for AdminAuditQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            before_audit_id: None,
            operation: None,
            principal_subject: None,
            result: None,
            correlation_id: None,
        }
    }
}

#[async_trait]
pub trait AdminAuditStore: Send + Sync {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError>;

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError>;
}
