use async_trait::async_trait;
use pmx_core::{AccountId, RemoteOrderId};
use serde_json::Value;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    LiveReadEventQuery, LiveReadEventRecord, LiveReadEventStore, StoreError,
    live_read_error_category_from_str, live_read_error_category_to_str,
    live_read_operation_from_str, live_read_operation_to_str, live_read_outcome_from_str,
    live_read_outcome_to_str, validate_live_read_event_for_store,
};

#[async_trait]
impl LiveReadEventStore for PostgresStore {
    async fn record_live_read_event(&self, event: &LiveReadEventRecord) -> Result<(), StoreError> {
        validate_live_read_event_for_store(event)?;
        let client = self.client().await?;
        let operation = live_read_operation_to_str(&event.operation);
        let outcome = live_read_outcome_to_str(&event.outcome);
        let remote_order_id = event.remote_order_id.as_ref().map(|id| id.0.clone());
        let error_category = event
            .error_category
            .as_ref()
            .map(live_read_error_category_to_str);
        let redacted_fields = serde_json::to_value(&event.redacted_fields)
            .map_err(|err| StoreError::InvalidData(err.to_string()))?;
        client
            .execute(
                "INSERT INTO live_read_events \
                 (account_id, operation, outcome, remote_order_id, remote_state, error_category, \
                  redacted_error_summary, no_trading_side_effect, redacted_fields) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE, $8)",
                &[
                    &event.account_id.0,
                    &operation,
                    &outcome,
                    &remote_order_id,
                    &event.remote_state,
                    &error_category,
                    &event.redacted_error_summary,
                    &redacted_fields,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn list_live_read_events(
        &self,
        query: &LiveReadEventQuery,
    ) -> Result<Vec<LiveReadEventRecord>, StoreError> {
        let client = self.client().await?;
        let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
        let account_id = query.account_id.as_ref().map(|id| id.0.clone());
        let operation = query.operation.as_ref().map(live_read_operation_to_str);
        let outcome = query.outcome.as_ref().map(live_read_outcome_to_str);
        let remote_order_id = query.remote_order_id.as_ref().map(|id| id.0.clone());
        let rows = client
            .query(
                "SELECT event_id, account_id, operation, outcome, remote_order_id, remote_state, \
                        error_category, redacted_error_summary, no_trading_side_effect, redacted_fields, observed_at
                 FROM live_read_events
                 WHERE ($2::bigint IS NULL OR event_id < $2)
                   AND ($3::text IS NULL OR account_id = $3)
                   AND ($4::text IS NULL OR operation = $4)
                   AND ($5::text IS NULL OR outcome = $5)
                   AND ($6::text IS NULL OR remote_order_id = $6)
                 ORDER BY event_id DESC
                 LIMIT $1",
                &[
                    &bounded_limit,
                    &query.before_event_id,
                    &account_id,
                    &operation,
                    &outcome,
                    &remote_order_id,
                ],
            )
            .await
            .map_err(map_db_error)?;
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let operation: String = row.get(2);
            let outcome: String = row.get(3);
            let remote_order_id: Option<String> = row.get(4);
            let error_category: Option<String> = row.get(6);
            let redacted_fields: Value = row.get(9);
            events.push(LiveReadEventRecord {
                event_id: Some(row.get(0)),
                account_id: AccountId(row.get(1)),
                operation: live_read_operation_from_str(&operation)?,
                outcome: live_read_outcome_from_str(&outcome)?,
                remote_order_id: remote_order_id.map(RemoteOrderId),
                remote_state: row.get(5),
                error_category: error_category
                    .as_deref()
                    .map(live_read_error_category_from_str)
                    .transpose()?,
                redacted_error_summary: row.get(7),
                no_trading_side_effect: row.get(8),
                redacted_fields: serde_json::from_value(redacted_fields)
                    .map_err(|err| StoreError::InvalidData(err.to_string()))?,
                observed_at: Some(row.get(10)),
            });
        }
        events.reverse();
        Ok(events)
    }
}
