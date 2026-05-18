use crate::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, IdempotencyAction, IdempotencyStore,
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
    RuntimeWorkerStatusStore, SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError,
    advisory_lock_key, sign_only_lifecycle_record_is_replay,
    validate_sign_only_lifecycle_append_for_store,
};
use async_trait::async_trait;
use chrono::Utc;
use pmx_core::SignOnlyLifecycleRecord;
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

#[async_trait]
impl RuntimeWorkerHealthStore for PostgresStore {
    async fn record_worker_heartbeat(
        &self,
        heartbeat: &RuntimeWorkerHeartbeat,
    ) -> Result<(), StoreError> {
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO worker_health \
                 (worker_id, role, capability, status, last_heartbeat_at, last_error, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, now()) \
                 ON CONFLICT (worker_id) DO UPDATE SET \
                   role = EXCLUDED.role, \
                   capability = EXCLUDED.capability, \
                   status = EXCLUDED.status, \
                   last_heartbeat_at = EXCLUDED.last_heartbeat_at, \
                   last_error = EXCLUDED.last_error, \
                   updated_at = now()",
                &[
                    &heartbeat.worker_id,
                    &heartbeat.role,
                    &heartbeat.capability,
                    &heartbeat.status,
                    &heartbeat.last_heartbeat_at,
                    &heartbeat.last_error,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerObservationStore for PostgresStore {
    async fn record_runtime_worker_observation(
        &self,
        observation: &RuntimeWorkerObservation,
    ) -> Result<(), StoreError> {
        let client = self.client().await?;
        let observed_at = observation.observed_at.unwrap_or_else(Utc::now);
        client
            .execute(
                "INSERT INTO runtime_worker_observations \
                 (account_id, capability, worker_kind, status, should_fail_closed, reason, observed_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &observation.account_id,
                    &observation.capability,
                    &observation.worker_kind,
                    &observation.status,
                    &observation.should_fail_closed,
                    &observation.reason,
                    &observed_at,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerStatusStore for PostgresStore {
    async fn list_runtime_worker_status(
        &self,
        query: &RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, StoreError> {
        let client = self.client().await?;
        let limit = query.bounded_limit() as i64;
        let heartbeat_rows = client
            .query(
                "SELECT worker_id, role, capability, status, last_heartbeat_at, last_error
                 FROM worker_health
                 ORDER BY last_heartbeat_at DESC, worker_id ASC
                 LIMIT $1",
                &[&limit],
            )
            .await
            .map_err(map_db_error)?;
        let heartbeats = heartbeat_rows
            .into_iter()
            .map(|row| RuntimeWorkerHeartbeat {
                worker_id: row.get(0),
                role: row.get(1),
                capability: row.get(2),
                status: row.get(3),
                last_heartbeat_at: row.get(4),
                last_error: row.get(5),
            })
            .collect();

        let observation_rows = client
            .query(
                "SELECT account_id, capability, worker_kind, status, should_fail_closed, reason, observed_at
                 FROM runtime_worker_observations
                 WHERE account_id = $1
                   AND ($2::timestamptz IS NULL OR observed_at < $2)
                 ORDER BY observed_at DESC, observation_id DESC
                 LIMIT $3",
                &[&query.account_id, &query.before_observed_at, &limit],
            )
            .await
            .map_err(map_db_error)?;
        let mut observations: Vec<_> = observation_rows
            .into_iter()
            .map(|row| RuntimeWorkerObservation {
                account_id: row.get(0),
                capability: row.get(1),
                worker_kind: row.get(2),
                status: row.get(3),
                should_fail_closed: row.get(4),
                reason: row.get(5),
                observed_at: Some(row.get(6)),
            })
            .collect();
        observations.reverse();
        Ok(RuntimeWorkerStatusReport {
            heartbeats,
            observations,
        })
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
