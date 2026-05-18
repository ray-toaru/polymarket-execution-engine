use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{IdempotencyAction, StoreError, advisory_lock_key};

pub(super) async fn begin_submit_attempt(
    store: &PostgresStore,
    account_id: &str,
    execution_id: &str,
    idempotency_key: &str,
    request_fingerprint: &str,
) -> Result<IdempotencyAction, StoreError> {
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
            "SELECT submit_attempt, request_fingerprint, response_fingerprint, response_json::text, status \
             FROM idempotency_records \
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

    if let Some(row) = row {
        let submit_attempt: i32 = row.get(0);
        let existing_request: String = row.get(1);
        let response_fingerprint: Option<String> = row.get(2);
        let response_json: Option<String> = row.get(3);
        let _status: String = row.get(4);
        if existing_request != request_fingerprint {
            PostgresStore::rollback(&client).await;
            return Ok(IdempotencyAction::Conflict);
        }
        PostgresStore::rollback(&client).await;
        if let (Some(response_fingerprint), Some(response_json)) =
            (response_fingerprint, response_json)
        {
            return Ok(IdempotencyAction::ReplayStoredResponse {
                response_fingerprint,
                response_json,
            });
        }
        return Ok(IdempotencyAction::InProgress {
            submit_attempt: submit_attempt as u32,
            retry_after_ms: 1_000,
        });
    }

    let row = match client
        .query_one(
            "SELECT COALESCE(MAX(submit_attempt), 0) + 1 \
             FROM idempotency_records \
             WHERE account_id = $1 AND execution_id = $2",
            &[&account_id, &execution_id],
        )
        .await
    {
        Ok(row) => row,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(map_db_error(err));
        }
    };
    let submit_attempt: i32 = row.get(0);
    let result = client
        .execute(
            "INSERT INTO idempotency_records \
             (account_id, execution_id, idempotency_key, submit_attempt, request_fingerprint, status) \
             VALUES ($1, $2, $3, $4, $5, 'PROCEEDING')",
            &[
                &account_id,
                &execution_id,
                &idempotency_key,
                &submit_attempt,
                &request_fingerprint,
            ],
        )
        .await;
    match result {
        Ok(_) => {
            client.batch_execute("COMMIT").await.map_err(map_db_error)?;
            Ok(IdempotencyAction::Proceed {
                submit_attempt: submit_attempt as u32,
                owner_token: format!("owner-{account_id}-{execution_id}-{submit_attempt}"),
            })
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            Err(map_db_error(err))
        }
    }
}
