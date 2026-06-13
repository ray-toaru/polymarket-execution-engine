use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{IdempotencyAction, StoreError, advisory_lock_key};
use chrono::{Duration, Utc};
use uuid::Uuid;

const IDEMPOTENCY_LEASE_SECS: i64 = 30;

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
            "SELECT submit_attempt, request_fingerprint, response_fingerprint, response_json::text, status, \
                    owner_token, lease_expires_at \
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
        let status: String = row.get(4);
        let _owner_token: Option<String> = row.get(5);
        let lease_expires_at: Option<chrono::DateTime<Utc>> = row.get(6);
        if existing_request != request_fingerprint {
            PostgresStore::rollback(&client).await;
            return Ok(IdempotencyAction::Conflict);
        }
        if let (Some(response_fingerprint), Some(response_json)) =
            (response_fingerprint, response_json)
        {
            PostgresStore::rollback(&client).await;
            return Ok(IdempotencyAction::ReplayStoredResponse {
                response_fingerprint,
                response_json,
            });
        }
        let now = Utc::now();
        if status == "PROCEEDING"
            && let Some(expires_at) = lease_expires_at
            && expires_at > now
        {
            PostgresStore::rollback(&client).await;
            return Ok(IdempotencyAction::InProgress {
                submit_attempt: submit_attempt as u32,
                retry_after_ms: (expires_at - now).num_milliseconds().max(1_000) as u64,
            });
        }

        let next_attempt = match next_submit_attempt(&client, account_id, execution_id).await {
            Ok(value) => value,
            Err(err) => {
                PostgresStore::rollback(&client).await;
                return Err(err);
            }
        };
        let owner_token = format!("owner-{}", Uuid::new_v4());
        let lease_expires_at = now + Duration::seconds(IDEMPOTENCY_LEASE_SECS);
        let result = client
            .execute(
                "UPDATE idempotency_records \
                 SET submit_attempt = $4, status = 'PROCEEDING', owner_token = $5, \
                     lease_expires_at = $6, response_fingerprint = NULL, response_json = NULL, updated_at = now() \
                 WHERE account_id = $1 AND execution_id = $2 AND idempotency_key = $3",
                &[
                    &account_id,
                    &execution_id,
                    &idempotency_key,
                    &next_attempt,
                    &owner_token,
                    &lease_expires_at,
                ],
            )
            .await;
        return match result {
            Ok(_) => {
                client.batch_execute("COMMIT").await.map_err(map_db_error)?;
                Ok(IdempotencyAction::Proceed {
                    submit_attempt: next_attempt as u32,
                    owner_token,
                })
            }
            Err(err) => {
                PostgresStore::rollback(&client).await;
                Err(map_db_error(err))
            }
        };
    }

    let submit_attempt = match next_submit_attempt(&client, account_id, execution_id).await {
        Ok(value) => value,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(err);
        }
    };
    let owner_token = format!("owner-{}", Uuid::new_v4());
    let lease_expires_at = Utc::now() + Duration::seconds(IDEMPOTENCY_LEASE_SECS);
    let result = client
        .execute(
            "INSERT INTO idempotency_records \
             (account_id, execution_id, idempotency_key, submit_attempt, request_fingerprint, status, owner_token, lease_expires_at) \
             VALUES ($1, $2, $3, $4, $5, 'PROCEEDING', $6, $7)",
            &[
                &account_id,
                &execution_id,
                &idempotency_key,
                &submit_attempt,
                &request_fingerprint,
                &owner_token,
                &lease_expires_at,
            ],
        )
        .await;
    match result {
        Ok(_) => {
            client.batch_execute("COMMIT").await.map_err(map_db_error)?;
            Ok(IdempotencyAction::Proceed {
                submit_attempt: submit_attempt as u32,
                owner_token,
            })
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            Err(map_db_error(err))
        }
    }
}

async fn next_submit_attempt(
    client: &tokio_postgres::Client,
    account_id: &str,
    execution_id: &str,
) -> Result<i32, StoreError> {
    let row = client
        .query_one(
            "SELECT COALESCE(MAX(submit_attempt), 0) + 1 \
             FROM idempotency_records \
             WHERE account_id = $1 AND execution_id = $2",
            &[&account_id, &execution_id],
        )
        .await
        .map_err(map_db_error)?;
    Ok(row.get(0))
}
