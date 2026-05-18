use super::InMemoryStore;
use async_trait::async_trait;
use chrono::Utc;

use crate::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, StoreError, sanitize_admin_audit_event,
};

#[async_trait]
impl AdminAuditStore for InMemoryStore {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.admin_audit_counter += 1;
        let mut stored = sanitize_admin_audit_event(event.clone());
        stored.audit_id = Some(state.admin_audit_counter);
        stored.created_at = Some(Utc::now());
        state.admin_audit.push(stored);
        Ok(())
    }

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .admin_audit
            .iter()
            .filter(|event| {
                query
                    .before_audit_id
                    .map(|before| event.audit_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .operation
                    .as_ref()
                    .map(|operation| &event.operation == operation)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .principal_subject
                    .as_ref()
                    .map(|principal_subject| &event.principal_subject == principal_subject)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .result
                    .as_ref()
                    .map(|result| &event.result == result)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .correlation_id
                    .as_ref()
                    .map(|correlation_id| event.correlation_id.as_ref() == Some(correlation_id))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.audit_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}
