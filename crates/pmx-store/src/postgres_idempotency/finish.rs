use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{FinishSubmitAttempt, StoreError, advisory_lock_key};

pub(super) async fn finish_submit_attempt(
    store: &PostgresStore,
    attempt: FinishSubmitAttempt<'_>,
) -> Result<(), StoreError> {
    let lock = advisory_lock_key("submit_attempt", attempt.account_id, attempt.execution_id);
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
            "SELECT request_fingerprint, owner_token, status FROM idempotency_records \
             WHERE account_id = $1 AND execution_id = $2 AND idempotency_key = $3",
            &[
                &attempt.account_id,
                &attempt.execution_id,
                &attempt.idempotency_key,
            ],
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
            "{account_id}/{execution_id}/{idempotency_key}",
            account_id = attempt.account_id,
            execution_id = attempt.execution_id,
            idempotency_key = attempt.idempotency_key
        )));
    };
    let existing_request: String = row.get(0);
    let existing_owner: Option<String> = row.get(1);
    let status: String = row.get(2);
    if existing_request != attempt.request_fingerprint {
        PostgresStore::rollback(&client).await;
        return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
    }
    if status != "PROCEEDING" || existing_owner.as_deref() != Some(attempt.owner_token) {
        PostgresStore::rollback(&client).await;
        return Err(StoreError::Conflict(
            "idempotency owner_token does not own proceeding attempt".into(),
        ));
    }
    let parsed_response: serde_json::Value = match serde_json::from_str(attempt.response_json) {
        Ok(value) => value,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(StoreError::InvalidData(err.to_string()));
        }
    };
    let result = client
        .execute(
            "UPDATE idempotency_records \
             SET response_fingerprint = $5, response_json = $6, status = 'DONE', \
                 lease_expires_at = NULL, updated_at = now() \
             WHERE account_id = $1 AND execution_id = $2 AND idempotency_key = $3 \
               AND owner_token = $4 AND status = 'PROCEEDING'",
            &[
                &attempt.account_id,
                &attempt.execution_id,
                &attempt.idempotency_key,
                &attempt.owner_token,
                &attempt.response_fingerprint,
                &parsed_response,
            ],
        )
        .await;
    match result {
        Ok(1) => {
            client.batch_execute("COMMIT").await.map_err(map_db_error)?;
            Ok(())
        }
        Ok(_) => {
            PostgresStore::rollback(&client).await;
            Err(StoreError::Conflict(
                "idempotency finish lost owner_token race".into(),
            ))
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            Err(map_db_error(err))
        }
    }
}
