use pmx_core::SignOnlyLifecycleRecord;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{SignOnlyLifecycleQuery, StoreError};

pub(super) async fn list_sign_only_lifecycle_events(
    store: &PostgresStore,
    query: &SignOnlyLifecycleQuery,
) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
    let client = store.client().await?;
    let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
    let rows = client
        .query(
            "SELECT payload, event_id, created_at
             FROM sign_only_lifecycle_events
             WHERE execution_id = $1
               AND ($2::bigint IS NULL OR event_id < $2)
             ORDER BY event_id DESC
             LIMIT $3",
            &[&query.execution_id, &query.before_event_id, &bounded_limit],
        )
        .await
        .map_err(map_db_error)?;
    let mut records: Vec<SignOnlyLifecycleRecord> = rows
        .into_iter()
        .map(|row| {
            let payload: serde_json::Value = row.get(0);
            let mut record: SignOnlyLifecycleRecord = serde_json::from_value(payload)
                .map_err(|err| StoreError::InvalidData(err.to_string()))?;
            record.event_id = Some(row.get(1));
            record.created_at = Some(row.get(2));
            Ok(record)
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    records.reverse();
    Ok(records)
}
