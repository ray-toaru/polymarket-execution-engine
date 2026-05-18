use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{StoreError, advisory_lock_key};

pub(super) async fn finish_submit_attempt(
    store: &PostgresStore,
    account_id: &str,
    execution_id: &str,
    idempotency_key: &str,
    request_fingerprint: &str,
    response_fingerprint: &str,
    response_json: &str,
) -> Result<(), StoreError> {
    let lock = advisory_lock_key("submit_attempt", account_id, execution_id);
    let client = store.client().await?;
    client.batch_execute("BEGIN").await.map_err(map_db_error)?;
    if let Err(err) = client
        .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
        .await
    {
        PostgresStore::rollback(&client).await;
        return Err(map_db_error(err));
    }
    let row = match client
        .query_opt(
            "SELECT request_fingerprint FROM idempotency_records \
             WHERE account_id = $1 AND execution_id = $2 AND idempotency_key = $3",
            &[&account_id, &execution_id, &idempotency_key],
        )
        .await
    {
        Ok(row) => row,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(map_db_error(err));
        }
    };
    let Some(row) = row else {
        PostgresStore::rollback(&client).await;
        return Err(StoreError::NotFound(format!(
            "{account_id}/{execution_id}/{idempotency_key}"
        )));
    };
    let existing_request: String = row.get(0);
    if existing_request != request_fingerprint {
        PostgresStore::rollback(&client).await;
        return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
    }
    let parsed_response: serde_json::Value =
        serde_json::from_str(response_json).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    let result = client
        .execute(
            "UPDATE idempotency_records \
             SET response_fingerprint = $4, response_json = $5, status = 'DONE', updated_at = now() \
             WHERE account_id = $1 AND execution_id = $2 AND idempotency_key = $3",
            &[
                &account_id,
                &execution_id,
                &idempotency_key,
                &response_fingerprint,
                &parsed_response,
            ],
        )
        .await;
    match result {
        Ok(_) => {
            client.batch_execute("COMMIT").await.map_err(map_db_error)?;
            Ok(())
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            Err(map_db_error(err))
        }
    }
}
