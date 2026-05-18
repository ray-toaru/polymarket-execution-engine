use crate::{IdempotencyAction, IdempotencyStore, StoreError, advisory_lock_key};
use async_trait::async_trait;
use tokio_postgres::{Client, NoTls};

use crate::postgres_support::map_db_error;

#[path = "postgres_migrations.rs"]
mod postgres_migrations;

/// PostgreSQL-backed execution store.
///
/// This implementation intentionally keeps connection management small and explicit for the
/// greenfield scaffold. Production code may replace it with a pool, but it must preserve the same
/// advisory-lock and SQL-constraint semantics tested here.
#[derive(Debug, Clone)]
pub struct PostgresStore {
    database_url: String,
}

impl PostgresStore {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    pub async fn connect(database_url: impl Into<String>) -> Result<Self, StoreError> {
        let store = Self::new(database_url);
        let client = store.client().await?;
        client
            .simple_query("SELECT 1")
            .await
            .map_err(map_db_error)?;
        Ok(store)
    }

    pub async fn apply_schema(&self) -> Result<(), StoreError> {
        let client = self.client().await?;
        postgres_migrations::apply_schema(&client).await
    }

    pub async fn applied_schema_migrations(&self) -> Result<Vec<(String, String)>, StoreError> {
        let client = self.client().await?;
        let rows = client
            .query(
                "SELECT version, checksum_sha256 FROM schema_migrations ORDER BY version",
                &[],
            )
            .await
            .map_err(map_db_error)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
            .collect())
    }

    pub(crate) async fn client(&self) -> Result<Client, StoreError> {
        let (client, connection) = tokio_postgres::connect(&self.database_url, NoTls)
            .await
            .map_err(map_db_error)?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                eprintln!("postgres connection task ended with error: {err}");
            }
        });
        Ok(client)
    }

    pub(crate) async fn rollback(client: &Client) {
        let _ = client.batch_execute("ROLLBACK").await;
    }
}

#[async_trait]
impl IdempotencyStore for PostgresStore {
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError> {
        let lock = advisory_lock_key("submit_attempt", account_id, execution_id);
        let client = self.client().await?;
        client.batch_execute("BEGIN").await.map_err(map_db_error)?;
        if let Err(err) = client
            .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
            .await
        {
            Self::rollback(&client).await;
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
                Self::rollback(&client).await;
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
                Self::rollback(&client).await;
                return Ok(IdempotencyAction::Conflict);
            }
            Self::rollback(&client).await;
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
                Self::rollback(&client).await;
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
                Self::rollback(&client).await;
                Err(map_db_error(err))
            }
        }
    }

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError> {
        let lock = advisory_lock_key("submit_attempt", account_id, execution_id);
        let client = self.client().await?;
        client.batch_execute("BEGIN").await.map_err(map_db_error)?;
        if let Err(err) = client
            .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
            .await
        {
            Self::rollback(&client).await;
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
                Self::rollback(&client).await;
                return Err(map_db_error(err));
            }
        };
        let Some(row) = row else {
            Self::rollback(&client).await;
            return Err(StoreError::NotFound(format!(
                "{account_id}/{execution_id}/{idempotency_key}"
            )));
        };
        let existing_request: String = row.get(0);
        if existing_request != request_fingerprint {
            Self::rollback(&client).await;
            return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
        }
        let parsed_response: serde_json::Value = serde_json::from_str(response_json)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
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
                Self::rollback(&client).await;
                Err(map_db_error(err))
            }
        }
    }
}

#[cfg(test)]
#[path = "postgres_tests.rs"]
mod postgres_tests;
