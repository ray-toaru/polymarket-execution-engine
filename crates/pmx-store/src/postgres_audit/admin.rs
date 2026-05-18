use async_trait::async_trait;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{AdminAuditEvent, AdminAuditQuery, AdminAuditStore, StoreError};

#[async_trait]
impl AdminAuditStore for PostgresStore {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError> {
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO admin_audit_events \
                 (principal_subject, operation, request_fingerprint, correlation_id, result) \
                 VALUES ($1, $2, $3, $4, $5)",
                &[
                    &event.principal_subject,
                    &event.operation,
                    &event.request_fingerprint,
                    &event.correlation_id,
                    &event.result,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError> {
        let client = self.client().await?;
        let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
        let rows = client
            .query(
                "SELECT audit_id, principal_subject, operation, request_fingerprint, correlation_id, result, created_at
                 FROM admin_audit_events
                 WHERE ($2::bigint IS NULL OR audit_id < $2)
                   AND ($3::text IS NULL OR operation = $3)
                   AND ($4::text IS NULL OR principal_subject = $4)
                   AND ($5::text IS NULL OR result = $5)
                   AND ($6::text IS NULL OR correlation_id = $6)
                 ORDER BY audit_id DESC
                 LIMIT $1",
                &[
                    &bounded_limit,
                    &query.before_audit_id,
                    &query.operation,
                    &query.principal_subject,
                    &query.result,
                    &query.correlation_id,
                ],
            )
            .await
            .map_err(map_db_error)?;
        let mut events: Vec<AdminAuditEvent> = rows
            .into_iter()
            .map(|row| AdminAuditEvent {
                audit_id: Some(row.get(0)),
                principal_subject: row.get(1),
                operation: row.get(2),
                request_fingerprint: row.get(3),
                correlation_id: row.get(4),
                result: row.get(5),
                created_at: Some(row.get(6)),
            })
            .collect();
        events.reverse();
        Ok(events)
    }
}
