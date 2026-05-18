use async_trait::async_trait;
use pmx_core::SignOnlyLifecycleRecord;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError, advisory_lock_key,
    sign_only_lifecycle_record_is_replay, validate_sign_only_lifecycle_append_for_store,
};

#[async_trait]
impl SignOnlyLifecycleStore for PostgresStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        let lock = advisory_lock_key(
            "sign_only_lifecycle",
            &record.account_id.0,
            &record.execution_id.0,
        );
        let client = self.client().await?;
        client.batch_execute("BEGIN").await.map_err(map_db_error)?;
        if let Err(err) = client
            .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
            .await
        {
            Self::rollback(&client).await;
            return Err(map_db_error(err));
        }

        let rows = match client
            .query(
                "SELECT payload, event_id, created_at FROM sign_only_lifecycle_events
                 WHERE execution_id = $1
                 ORDER BY event_id ASC",
                &[&record.execution_id.0],
            )
            .await
        {
            Ok(rows) => rows,
            Err(err) => {
                Self::rollback(&client).await;
                return Err(map_db_error(err));
            }
        };

        let existing: Vec<SignOnlyLifecycleRecord> = match rows
            .into_iter()
            .map(|row| {
                let payload: serde_json::Value = row.get(0);
                let mut record: SignOnlyLifecycleRecord = serde_json::from_value(payload)
                    .map_err(|err| StoreError::InvalidData(err.to_string()))?;
                record.event_id = Some(row.get(1));
                record.created_at = Some(row.get(2));
                Ok(record)
            })
            .collect::<Result<Vec<_>, StoreError>>()
        {
            Ok(existing) => existing,
            Err(err) => {
                Self::rollback(&client).await;
                return Err(err);
            }
        };

        match sign_only_lifecycle_record_is_replay(&existing, record) {
            Ok(true) => {
                client.batch_execute("COMMIT").await.map_err(map_db_error)?;
                return Ok(());
            }
            Ok(false) => {}
            Err(err) => {
                Self::rollback(&client).await;
                return Err(err);
            }
        }
        if let Err(err) = validate_sign_only_lifecycle_append_for_store(&existing, record) {
            Self::rollback(&client).await;
            return Err(err);
        }

        let mut stored = record.clone();
        stored.event_id = None;
        stored.created_at = None;
        let payload = serde_json::to_value(&stored)
            .map_err(|err| StoreError::InvalidData(err.to_string()))?;
        let result = client
            .execute(
                "INSERT INTO sign_only_lifecycle_events \
                 (execution_id, account_id, state, event_type, client_event_id, signed_order_ref, no_remote_side_effect, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &stored.execution_id.0,
                    &stored.account_id.0,
                    &format!("{:?}", stored.state),
                    &format!("{:?}", stored.event),
                    &stored.client_event_id,
                    &stored.signed_order_ref,
                    &stored.no_remote_side_effect,
                    &payload,
                ],
            )
            .await;
        match result {
            Ok(_) => {
                client.batch_execute("COMMIT").await.map_err(map_db_error)?;
                Ok(())
            }
            Err(err) => {
                Self::rollback(&client).await;
                Err(map_db_error(err))
            }
        }
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        let client = self.client().await?;
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
}
