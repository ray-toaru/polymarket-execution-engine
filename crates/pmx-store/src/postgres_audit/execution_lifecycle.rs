use async_trait::async_trait;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    ExecutionLifecycleEvent, ExecutionLifecycleQuery, ExecutionLifecycleStore, StoreError,
};

#[async_trait]
impl ExecutionLifecycleStore for PostgresStore {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload = event.payload.clone();
        client
            .execute(
                "INSERT INTO execution_lifecycle_events \
                 (execution_id, account_id, event_type, event_source, payload) \
                 VALUES ($1, $2, $3, $4, $5)",
                &[
                    &event.execution_id,
                    &event.account_id,
                    &event.event_type,
                    &event.event_source,
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
        let client = self.client().await?;
        let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
        let rows = client
            .query(
                "SELECT event_id, execution_id, account_id, event_type, event_source, payload, created_at
                 FROM execution_lifecycle_events
                 WHERE execution_id = $1
                   AND ($2::bigint IS NULL OR event_id < $2)
                 ORDER BY event_id DESC
                 LIMIT $3",
                &[&query.execution_id, &query.before_event_id, &bounded_limit],
            )
            .await
            .map_err(map_db_error)?;
        let mut events: Vec<ExecutionLifecycleEvent> = rows
            .into_iter()
            .map(|row| ExecutionLifecycleEvent {
                event_id: Some(row.get(0)),
                execution_id: row.get(1),
                account_id: row.get(2),
                event_type: row.get(3),
                event_source: row.get(4),
                payload: row.get(5),
                created_at: Some(row.get(6)),
            })
            .collect();
        events.reverse();
        Ok(events)
    }
}
