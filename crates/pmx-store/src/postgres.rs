use crate::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore, OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord,
    OrderLifecycleStore, RuntimeStateQuery, RuntimeStateStore, RuntimeWorkerHealthStore,
    RuntimeWorkerHeartbeat, RuntimeWorkerObservation, RuntimeWorkerObservationStore,
    SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError, advisory_lock_key,
    apply_runtime_worker_observations, order_event_kind_from_str, order_event_kind_to_str,
    order_lifecycle_state_from_str, order_lifecycle_state_to_str,
    quantity_bound_to_resource_and_amount, reservation_state_to_str,
    runtime_observation_ttl_seconds, sign_only_lifecycle_record_is_replay, submit_status_str,
    validate_sign_only_lifecycle_append_for_store,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
#[cfg(test)]
use pmx_core::sign_only_lifecycle_records_equivalent;
use pmx_core::{
    CollateralProfileStatus, ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot,
    GeoblockStatus, NormalizedIntent, OrderReservation, RuntimeStateSummary,
    SignOnlyLifecycleRecord, SubmitReceipt, WorkerStatus, transition_order_state,
};
use tokio_postgres::{Client, NoTls};

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
        client
            .batch_execute(include_str!("../../../migrations/0001_initial.sql"))
            .await
            .map_err(map_db_error)
    }

    async fn client(&self) -> Result<Client, StoreError> {
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

    async fn rollback(client: &Client) {
        let _ = client.batch_execute("ROLLBACK").await;
    }
}

fn map_db_error(err: tokio_postgres::Error) -> StoreError {
    if let Some(db_error) = err.as_db_error() {
        if db_error.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION {
            return StoreError::NotFound(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::CHECK_VIOLATION {
            return StoreError::Conflict(db_error.message().to_string());
        }
        if db_error.code() == &tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE {
            return StoreError::SerializationFailure;
        }
    }
    StoreError::DatabaseUnavailable(err.to_string())
}

async fn load_json_payload<T: serde::de::DeserializeOwned>(
    client: &Client,
    table: &str,
    id_column: &str,
    id_value: &str,
    payload_column: &str,
) -> Result<T, StoreError> {
    let query = format!("SELECT {payload_column} FROM {table} WHERE {id_column} = $1");
    let row = client
        .query_opt(&query, &[&id_value])
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| StoreError::NotFound(format!("{table}.{id_column}={id_value}")))?;
    let payload: serde_json::Value = row.get(0);
    serde_json::from_value(payload).map_err(|err| StoreError::InvalidData(err.to_string()))
}

fn geoblock_from_runtime_account_status(status: Option<&str>) -> GeoblockStatus {
    match status.map(|s| s.trim().to_ascii_uppercase()) {
        Some(s) if matches!(s.as_str(), "ACTIVE" | "ALLOWED" | "READY") => GeoblockStatus::Allowed,
        Some(s) if matches!(s.as_str(), "BLOCKED" | "GEO_BLOCKED" | "GEOBLOCKED") => {
            GeoblockStatus::Blocked
        }
        Some(s) if s == "ERROR" => GeoblockStatus::Error,
        _ => GeoblockStatus::Unknown,
    }
}

fn collateral_status_from_db(
    status: Option<&str>,
    explicit_profile: bool,
) -> CollateralProfileStatus {
    match status.map(|s| s.trim().to_ascii_uppercase()) {
        Some(s) if s == "RESOLVED" => CollateralProfileStatus::Resolved,
        Some(s) if matches!(s.as_str(), "DEFAULT" | "DEFAULT_RESOLVED") => {
            CollateralProfileStatus::DefaultResolved
        }
        Some(s) if matches!(s.as_str(), "MISSING" | "EXPLICIT_MISSING") => {
            CollateralProfileStatus::ExplicitMissing
        }
        None if explicit_profile => CollateralProfileStatus::ExplicitMissing,
        _ => CollateralProfileStatus::Unknown,
    }
}

fn worker_status_from_rows(
    rows: &[(String, chrono::DateTime<Utc>)],
    expected: usize,
) -> WorkerStatus {
    if expected == 0 {
        return WorkerStatus::Healthy;
    }
    if rows.len() < expected {
        return WorkerStatus::Unknown;
    }
    let stale_cutoff = Utc::now() - Duration::seconds(120);
    let mut degraded = false;
    for (status, last_heartbeat_at) in rows {
        let normalized = status.trim().to_ascii_uppercase();
        if matches!(normalized.as_str(), "STALE" | "ERROR" | "DOWN")
            || *last_heartbeat_at < stale_cutoff
        {
            return WorkerStatus::Stale;
        }
        if normalized == "DEGRADED" {
            degraded = true;
        } else if normalized != "HEALTHY" {
            return WorkerStatus::Unknown;
        }
    }
    if degraded {
        WorkerStatus::Degraded
    } else {
        WorkerStatus::Healthy
    }
}

#[async_trait]
impl RuntimeStateStore for PostgresStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let client = self.client().await?;
        let account_row = client
            .query_opt(
                "SELECT status, kill_switch_enabled FROM runtime_accounts WHERE account_id = $1",
                &[&query.account_id],
            )
            .await
            .map_err(map_db_error)?;
        let (account_status, kill_switch_enabled) = if let Some(row) = account_row {
            (Some(row.get::<_, String>(0)), row.get::<_, bool>(1))
        } else {
            (None, true)
        };

        let geoblock_status = geoblock_from_runtime_account_status(account_status.as_deref());

        let collateral_profile_status = if let Some(profile_id) = &query.collateral_profile_id {
            let row = client
                .query_opt(
                    "SELECT status FROM collateral_profiles WHERE profile_id = $1",
                    &[profile_id],
                )
                .await
                .map_err(map_db_error)?;
            let status = row.map(|row| row.get::<_, String>(0));
            collateral_status_from_db(status.as_deref(), true)
        } else {
            let row = client
                .query_opt(
                    "SELECT status FROM collateral_profiles WHERE status IN ('DEFAULT', 'DEFAULT_RESOLVED', 'RESOLVED') ORDER BY created_at DESC LIMIT 1",
                    &[],
                )
                .await
                .map_err(map_db_error)?;
            let status = row.map(|row| row.get::<_, String>(0));
            collateral_status_from_db(status.as_deref(), false)
        };

        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ];
        }
        let mut worker_rows = Vec::new();
        for capability in &required_capabilities {
            if let Some(row) = client
                .query_opt(
                    "SELECT status, last_heartbeat_at FROM worker_health WHERE capability = $1 ORDER BY updated_at DESC LIMIT 1",
                    &[capability],
                )
                .await
                .map_err(map_db_error)?
            {
                worker_rows.push((
                    row.get::<_, String>(0),
                    row.get::<_, chrono::DateTime<Utc>>(1),
                ));
            }
        }

        let base = RuntimeStateSummary {
            geoblock_status,
            worker_status: worker_status_from_rows(&worker_rows, required_capabilities.len()),
            collateral_profile_status,
            kill_switch_enabled,
            required_capabilities,
        };
        let observation_ttl_seconds: i32 = runtime_observation_ttl_seconds() as i32;
        let observation_rows = client
            .query(
                "SELECT DISTINCT ON (capability)
                    account_id, capability, worker_kind, status, should_fail_closed, reason, observed_at
                 FROM runtime_worker_observations
                 WHERE account_id = $1
                   AND observed_at >= now() - ($2::integer * interval '1 second')
                 ORDER BY capability, observed_at DESC, observation_id DESC",
                &[&query.account_id, &observation_ttl_seconds],
            )
            .await
            .map_err(map_db_error)?;
        let observations: Vec<RuntimeWorkerObservation> = observation_rows
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
        Ok(apply_runtime_worker_observations(base, &observations))
    }
}

#[async_trait]
impl ExecutionStore for PostgresStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(intent).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (normalized_intent_id) DO UPDATE SET payload = EXCLUDED.payload",
                &[&intent.normalized_intent_id, &intent.intent_hash.0, &intent.account_id.0, &payload],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "normalized_intents",
            "normalized_intent_id",
            normalized_intent_id,
            "payload",
        )
        .await
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(snapshot).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (snapshot_id) DO UPDATE SET payload = EXCLUDED.payload",
                &[
                    &snapshot.snapshot_id,
                    &snapshot.snapshot_hash.0,
                    &snapshot.normalized_intent_id,
                    &payload,
                    &snapshot.captured_at,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "feasibility_snapshots",
            "snapshot_id",
            snapshot_id,
            "payload",
        )
        .await
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(decision).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let reasons = serde_json::to_value(&decision.reasons)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let snapshot_id: Option<String> = None;
        client
            .execute(
                "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (decision_id) DO UPDATE SET status = EXCLUDED.status, reasons = EXCLUDED.reasons, payload = EXCLUDED.payload",
                &[
                    &decision.decision_id,
                    &decision.decision_hash.0,
                    &snapshot_id,
                    &format!("{:?}", decision.status).to_uppercase(),
                    &reasons,
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "constraint_decisions",
            "decision_id",
            decision_id,
            "payload",
        )
        .await
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(plan).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO execution_plans \
                 (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (execution_id) DO UPDATE SET \
                   account_id = EXCLUDED.account_id, \
                   normalized_intent_id = EXCLUDED.normalized_intent_id, \
                   snapshot_id = EXCLUDED.snapshot_id, \
                   decision_id = EXCLUDED.decision_id, \
                   plan_hash = EXCLUDED.plan_hash, \
                   status = EXCLUDED.status, \
                   summary_json = EXCLUDED.summary_json, \
                   updated_at = now()",
                &[
                    &plan.execution_id,
                    &plan.account_id.0,
                    &plan.normalized_intent_id,
                    &plan.snapshot_id,
                    &plan.decision_id,
                    &plan.plan_hash.0,
                    &format!("{:?}", plan.status).to_uppercase(),
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "execution_plans",
            "execution_id",
            execution_id,
            "summary_json",
        )
        .await
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        let (resource_kind, amount) =
            quantity_bound_to_resource_and_amount(&reservation.quantity_bound)?;
        let lock = advisory_lock_key(
            "reservation",
            &reservation.account_id.0,
            &format!("{}:{resource_kind}", reservation.execution_id.0),
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
        let order_id: Option<&str> = reservation.internal_order_id.as_ref().map(|v| v.0.as_str());
        let result = client
            .execute(
                "INSERT INTO order_reservations (reservation_id, order_id, execution_id, account_id, resource_kind, amount, state) \
                 VALUES ($1, $2, $3, $4, $5, $6::text::numeric, $7) \
                 ON CONFLICT (reservation_id) DO UPDATE SET state = EXCLUDED.state",
                &[
                    &reservation.reservation_id,
                    &order_id,
                    &reservation.execution_id.0,
                    &reservation.account_id.0,
                    &resource_kind,
                    &amount,
                    &reservation_state_to_str(&reservation.state),
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

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        let client = self.client().await?;
        let payload =
            serde_json::to_value(receipt).map_err(|e| StoreError::InvalidData(e.to_string()))?;
        client
            .execute(
                "INSERT INTO submit_receipts (execution_id, receipt_id, status, executor_version, contract_version, response_json) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (execution_id) DO UPDATE SET receipt_id = EXCLUDED.receipt_id, status = EXCLUDED.status, response_json = EXCLUDED.response_json, updated_at = now()",
                &[
                    &receipt.execution_id,
                    &receipt.receipt_id,
                    &submit_status_str(&receipt.status),
                    &receipt.executor_version,
                    &receipt.contract_version,
                    &payload,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        let client = self.client().await?;
        load_json_payload(
            &client,
            "submit_receipts",
            "execution_id",
            execution_id,
            "response_json",
        )
        .await
    }
}

#[async_trait]
impl OrderLifecycleStore for PostgresStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO orders \
                 (order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now()) \
                 ON CONFLICT (order_id) DO UPDATE SET \
                   execution_id = EXCLUDED.execution_id, \
                   account_id = EXCLUDED.account_id, \
                   condition_id = EXCLUDED.condition_id, \
                   token_id = EXCLUDED.token_id, \
                   side = EXCLUDED.side, \
                   lifecycle_state = EXCLUDED.lifecycle_state, \
                   remote_order_id = EXCLUDED.remote_order_id, \
                   remote_state = EXCLUDED.remote_state, \
                   updated_at = now()",
                &[
                    &order.order_id,
                    &order.execution_id,
                    &order.account_id,
                    &order.condition_id,
                    &order.token_id,
                    &order.side,
                    &order_lifecycle_state_to_str(&order.lifecycle_state),
                    &order.remote_order_id,
                    &order.remote_state,
                ],
            )
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        let lock = advisory_lock_key("order_lifecycle", "order", &event.order_id);
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
                "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
                 FROM orders
                 WHERE order_id = $1",
                &[&event.order_id],
            )
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                Self::rollback(&client).await;
                return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
            }
            Err(err) => {
                Self::rollback(&client).await;
                return Err(map_db_error(err));
            }
        };
        let current_state: String = row.get(6);
        let current = order_lifecycle_state_from_str(&current_state)?;
        let next = match transition_order_state(current, event.event.clone()) {
            Ok(next) => next,
            Err(err) => {
                Self::rollback(&client).await;
                return Err(StoreError::Conflict(err.to_string()));
            }
        };
        let payload = event.payload.clone();
        if let Err(err) = client
            .execute(
                "INSERT INTO order_events (order_id, event_type, event_source, payload) VALUES ($1, $2, $3, $4)",
                &[&event.order_id, &order_event_kind_to_str(&event.event), &event.event_source, &payload],
            )
            .await
        {
            Self::rollback(&client).await;
            return Err(map_db_error(err));
        }
        if let Err(err) = client
            .execute(
                "UPDATE orders SET lifecycle_state = $2, updated_at = now() WHERE order_id = $1",
                &[&event.order_id, &order_lifecycle_state_to_str(&next)],
            )
            .await
        {
            Self::rollback(&client).await;
            return Err(map_db_error(err));
        }
        client.batch_execute("COMMIT").await.map_err(map_db_error)?;
        Ok(OrderLifecycleRecord {
            order_id: row.get(0),
            execution_id: row.get(1),
            account_id: row.get(2),
            condition_id: row.get(3),
            token_id: row.get(4),
            side: row.get(5),
            lifecycle_state: next,
            remote_order_id: row.get(7),
            remote_state: row.get(8),
            created_at: Some(row.get(9)),
            updated_at: Some(Utc::now()),
        })
    }

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
                 FROM orders
                 WHERE order_id = $1",
                &[&order_id],
            )
            .await
            .map_err(map_db_error)?;
        row.map(|row| {
            let state: String = row.get(6);
            Ok(OrderLifecycleRecord {
                order_id: row.get(0),
                execution_id: row.get(1),
                account_id: row.get(2),
                condition_id: row.get(3),
                token_id: row.get(4),
                side: row.get(5),
                lifecycle_state: order_lifecycle_state_from_str(&state)?,
                remote_order_id: row.get(7),
                remote_state: row.get(8),
                created_at: Some(row.get(9)),
                updated_at: Some(row.get(10)),
            })
        })
        .transpose()
    }

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        let client = self.client().await?;
        let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
        let rows = client
            .query(
                "SELECT event_id, order_id, event_type, event_source, payload, created_at
                 FROM order_events
                 WHERE order_id = $1
                   AND ($2::bigint IS NULL OR event_id < $2)
                 ORDER BY event_id DESC
                 LIMIT $3",
                &[&query.order_id, &query.before_event_id, &bounded_limit],
            )
            .await
            .map_err(map_db_error)?;
        let mut events: Vec<OrderLifecycleEventRecord> = rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get(2);
                Ok(OrderLifecycleEventRecord {
                    event_id: Some(row.get(0)),
                    order_id: row.get(1),
                    event: order_event_kind_from_str(&event_type)?,
                    event_source: row.get(3),
                    payload: row.get(4),
                    created_at: Some(row.get(5)),
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        events.reverse();
        Ok(events)
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
mod tests {
    use super::*;
    use pmx_core::{
        AccountId, DecimalString, ExecutionId, OrderReservation, QuantityBound, ReservationState,
        SubmitReceipt, SubmitStatus,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    async fn test_store() -> Option<PostgresStore> {
        let Ok(url) = std::env::var("PMX_TEST_DATABASE_URL") else {
            eprintln!("PMX_TEST_DATABASE_URL not set; skipping PostgreSQL repository test");
            return None;
        };
        let store = PostgresStore::connect(url).await.ok()?;
        store.apply_schema().await.ok()?;
        Some(store)
    }

    fn unique(prefix: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        format!("{prefix}-{nanos}")
    }

    pub(super) async fn seed_execution_plan(
        store: &PostgresStore,
        account_id: &str,
        execution_id: &str,
    ) {
        let client = store.client().await.expect("test postgres client");
        let norm = unique("norm");
        let snap = unique("snap");
        let dec = unique("decision");
        let plan_hash = unique("plan-hash");
        client
            .execute(
                "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
                 VALUES ($1, $2, $3, '{}'::jsonb)",
                &[&norm, &unique("intent-hash"), &account_id],
            )
            .await
            .expect("seed normalized intent");
        client
            .execute(
                "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
                 VALUES ($1, $2, $3, '{}'::jsonb, now())",
                &[&snap, &unique("snapshot-hash"), &norm],
            )
            .await
            .expect("seed snapshot");
        client
            .execute(
                "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
                 VALUES ($1, $2, $3, 'ALLOW', '[]'::jsonb, '{}'::jsonb)",
                &[&dec, &unique("decision-hash"), &snap],
            )
            .await
            .expect("seed decision");
        client
            .execute(
                "INSERT INTO execution_plans (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
                 VALUES ($1, $2, $3, $4, $5, $6, 'READY', '{}'::jsonb)",
                &[&execution_id, &account_id, &norm, &snap, &dec, &plan_hash],
            )
            .await
            .expect("seed execution plan");
    }

    #[tokio::test]
    async fn postgres_records_admin_audit_event() {
        let Some(store) = test_store().await else {
            return;
        };
        let principal = unique("principal");
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: principal.clone(),
                operation: "KillSwitch".into(),
                request_fingerprint: Some(unique("request-fp")),
                correlation_id: Some(unique("corr")),
                result: "ACCEPTED".into(),
                created_at: None,
            })
            .await
            .expect("record audit event");
        let client = store.client().await.expect("test postgres client");
        let row = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM admin_audit_events WHERE principal_subject = $1",
                &[&principal],
            )
            .await
            .expect("count audit events");
        let count: i64 = row.get(0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn same_request_replay_is_persisted() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        super::tests::seed_execution_plan(&store, &account, &execution).await;
        let action = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("begin idempotency");
        assert_eq!(
            action,
            IdempotencyAction::Proceed {
                submit_attempt: 1,
                owner_token: format!("owner-{account}-{execution}-1"),
            }
        );
        store
            .finish_submit_attempt(
                &account,
                &execution,
                "idem-1",
                "req-1",
                "resp-1",
                r#"{"status":"accepted"}"#,
            )
            .await
            .expect("finish idempotency");
        let replay = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("replay idempotency");
        assert!(matches!(
            replay,
            IdempotencyAction::ReplayStoredResponse { .. }
        ));
    }

    #[tokio::test]
    async fn fingerprint_mismatch_is_conflict() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-1")
            .await
            .expect("begin idempotency");
        let conflict = store
            .begin_submit_attempt(&account, &execution, "idem-1", "req-2")
            .await
            .expect("conflict result");
        assert_eq!(conflict, IdempotencyAction::Conflict);
    }

    #[tokio::test]
    async fn in_progress_replay_does_not_return_proceed() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        let first = store
            .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
            .await
            .expect("first begin");
        assert!(matches!(first, IdempotencyAction::Proceed { .. }));
        let second = store
            .begin_submit_attempt(&account, &execution, "idem-progress", "req-progress")
            .await
            .expect("second begin");
        assert!(matches!(second, IdempotencyAction::InProgress { .. }));
    }

    #[tokio::test]
    async fn remote_unknown_is_persisted_conservatively() {
        let Some(store) = test_store().await else {
            return;
        };
        let execution = unique("exec");
        let receipt = SubmitReceipt {
            execution_id: execution.clone(),
            receipt_id: unique("receipt"),
            status: SubmitStatus::RemoteUnknown,
            executor_version: "test".into(),
            contract_version: "test".into(),
        };
        store
            .record_submit_receipt(&receipt)
            .await
            .expect("record remote unknown receipt");
        let client = store.client().await.expect("test postgres client");
        let status: String = client
            .query_one(
                "SELECT status FROM submit_receipts WHERE execution_id = $1",
                &[&execution],
            )
            .await
            .expect("query receipt")
            .get(0);
        assert_eq!(status, "REMOTE_UNKNOWN");
    }

    #[tokio::test]
    async fn reservation_double_spend_is_prevented_concurrently() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct");
        let execution = unique("exec");
        seed_execution_plan(&store, &account, &execution).await;
        let reservation = OrderReservation {
            reservation_id: unique("reservation"),
            account_id: AccountId(account.clone()),
            execution_id: ExecutionId(execution.clone()),
            internal_order_id: None,
            quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString("10".into())),
            state: ReservationState::Active,
        };
        let a = store.clone();
        let b = store.clone();
        let r1 = reservation.clone();
        let r2 = reservation;
        let (left, right) = tokio::join!(
            async move { a.save_order_reservation(&r1).await },
            async move { b.save_order_reservation(&r2).await }
        );
        assert!(left.is_ok() || right.is_ok());
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM order_reservations WHERE account_id = $1 AND execution_id = $2",
                &[&account, &execution],
            )
            .await
            .expect("query reservations")
            .get(0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn postgres_records_execution_lifecycle_event() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-life");
        let execution = unique("exec-life");
        seed_execution_plan(&store, &account, &execution).await;
        store
            .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                event_id: None,
                execution_id: execution.clone(),
                account_id: account.clone(),
                event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
                event_source: "pmx-service".into(),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record lifecycle event");
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM execution_lifecycle_events WHERE execution_id = $1 AND event_type = 'SUBMIT_BLOCKED_BEFORE_REMOTE'",
                &[&execution],
            )
            .await
            .expect("count lifecycle events")
            .get(0);
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn postgres_loads_runtime_state_from_runtime_tables() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-runtime");
        let condition = unique("cond-runtime");
        let profile = unique("profile-runtime");
        let client = store.client().await.expect("test postgres client");
        client
            .execute(
                "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
                &[&account],
            )
            .await
            .expect("seed runtime account");
        client
            .execute(
                "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
                &[&condition],
            )
            .await
            .expect("seed runtime market");
        client
            .execute(
                "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
                 VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
                &[&profile],
            )
            .await
            .expect("seed collateral profile");
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            let worker_id = unique(&format!("worker-{capability}"));
            let capability_value = capability.to_string();
            client
                .execute(
                    "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                     VALUES ($1, 'test', $2, 'HEALTHY', now())",
                    &[&worker_id, &capability_value],
                )
                .await
                .expect("seed worker health");
        }
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: account,
                condition_id: condition,
                collateral_profile_id: Some(profile),
                required_capabilities: vec![
                    "heartbeat".into(),
                    "reconcile".into(),
                    "resource-refresh".into(),
                ],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.geoblock_status, GeoblockStatus::Allowed);
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
        assert_eq!(
            state.collateral_profile_status,
            CollateralProfileStatus::Resolved
        );
        assert!(!state.kill_switch_enabled);
    }

    #[tokio::test]
    async fn postgres_persists_sign_only_lifecycle_records() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-sign-only");
        let execution = unique("exec-sign-only");
        seed_execution_plan(&store, &account, &execution).await;
        let records_to_append = [
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
                event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::SigningRequested,
                event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: pmx_core::ExecutionId(execution.clone()),
                account_id: pmx_core::AccountId(account.clone()),
                state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
                event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
                client_event_id: None,
                signed_order_ref: Some("sign-only:redacted-ref".into()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
        ];
        for record in &records_to_append {
            store
                .record_sign_only_lifecycle_event(record)
                .await
                .expect("record sign-only lifecycle event");
        }
        store
            .record_sign_only_lifecycle_event(records_to_append.last().unwrap())
            .await
            .expect("replay terminal sign-only lifecycle event");
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: execution.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle events");
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.event_id.is_some()));
        assert!(records.iter().all(|record| record.created_at.is_some()));
        assert!(sign_only_lifecycle_records_equivalent(
            records.last().unwrap(),
            records_to_append.last().unwrap()
        ));
    }

    #[tokio::test]
    async fn postgres_runtime_worker_observations_degrade_runtime_state() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-runtime-observed");
        let condition = unique("cond-runtime-observed");
        let profile = unique("profile-runtime-observed");
        let client = store.client().await.expect("test postgres client");
        client
            .execute(
                "INSERT INTO runtime_accounts (account_id, status, kill_switch_enabled) VALUES ($1, 'ACTIVE', false)",
                &[&account],
            )
            .await
            .expect("seed runtime account");
        client
            .execute(
                "INSERT INTO runtime_markets (condition_id, status, is_sports) VALUES ($1, 'ACTIVE', false)",
                &[&condition],
            )
            .await
            .expect("seed runtime market");
        client
            .execute(
                "INSERT INTO collateral_profiles (profile_id, status, quote_asset_symbol, quote_asset_address, allowance_target, decimals, profile_version) \
                 VALUES ($1, 'RESOLVED', 'pUSD', '0x0000000000000000000000000000000000000001', '0x0000000000000000000000000000000000000002', 6, 'test')",
                &[&profile],
            )
            .await
            .expect("seed collateral profile");
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            let worker_id = unique(&format!("worker-{capability}"));
            let capability_value = capability.to_string();
            client
                .execute(
                    "INSERT INTO worker_health (worker_id, role, capability, status, last_heartbeat_at) \
                     VALUES ($1, 'test', $2, 'HEALTHY', now())",
                    &[&worker_id, &capability_value],
                )
                .await
                .expect("seed worker health");
        }
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: account.clone(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record runtime worker observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: account,
                condition_id: condition,
                collateral_profile_id: Some(profile),
                required_capabilities: vec![
                    "heartbeat".into(),
                    "reconcile".into(),
                    "resource-refresh".into(),
                ],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
        assert!(
            state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    #[tokio::test]
    async fn postgres_records_cancel_reconcile_lifecycle_events() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-cancel-life");
        let execution = unique("exec-cancel-life");
        seed_execution_plan(&store, &account, &execution).await;
        for event_type in ["CANCEL_REQUESTED_NON_LIVE", "RECONCILE_REQUESTED_NON_LIVE"] {
            store
                .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                    event_id: None,
                    execution_id: execution.clone(),
                    account_id: account.clone(),
                    event_type: event_type.into(),
                    event_source: "pmx-store-test".into(),
                    payload: serde_json::json!({"no_remote_side_effect": true}),
                    created_at: None,
                })
                .await
                .expect("record lifecycle event");
        }
        let events = store
            .list_execution_lifecycle_events(&ExecutionLifecycleQuery {
                execution_id: execution.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list lifecycle events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "CANCEL_REQUESTED_NON_LIVE");
        assert_eq!(events[1].event_type, "RECONCILE_REQUESTED_NON_LIVE");
    }

    #[tokio::test]
    async fn postgres_records_runtime_worker_observation() {
        let Some(store) = test_store().await else {
            return;
        };
        let account = unique("acct-worker-observation");
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: account.clone(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record runtime worker observation");
        let client = store.client().await.expect("test postgres client");
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM runtime_worker_observations WHERE account_id = $1",
                &[&account],
            )
            .await
            .expect("count runtime worker observations")
            .get(0);
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod runtime_worker_health_pg_tests_v23 {
    use super::*;
    use chrono::Utc;

    async fn test_store() -> Option<PostgresStore> {
        let Ok(url) = std::env::var("PMX_TEST_DATABASE_URL") else {
            eprintln!("PMX_TEST_DATABASE_URL not set; skipping PostgreSQL heartbeat test");
            return None;
        };
        let store = PostgresStore::connect(url).await.ok()?;
        store.apply_schema().await.ok()?;
        Some(store)
    }

    #[tokio::test]
    async fn postgres_records_worker_heartbeat() {
        let Some(store) = test_store().await else {
            return;
        };
        let worker_id = format!(
            "worker-heartbeat-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: worker_id.clone(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        let client = store.client().await.expect("test postgres client");
        let row = client
            .query_one(
                "SELECT status FROM worker_health WHERE worker_id = $1",
                &[&worker_id],
            )
            .await
            .expect("heartbeat row");
        let status: String = row.get(0);
        assert_eq!(status, "HEALTHY");
    }
}

#[cfg(test)]
mod order_lifecycle_pg_tests_v23 {
    use super::*;
    use crate::postgres::tests::seed_execution_plan;
    use pmx_core::{OrderEventKind, OrderLifecycleState};

    async fn test_store() -> Option<PostgresStore> {
        let Ok(url) = std::env::var("PMX_TEST_DATABASE_URL") else {
            eprintln!("PMX_TEST_DATABASE_URL not set; skipping PostgreSQL order lifecycle test");
            return None;
        };
        let store = PostgresStore::connect(url).await.ok()?;
        store.apply_schema().await.ok()?;
        Some(store)
    }

    #[tokio::test]
    async fn postgres_records_order_lifecycle_event() {
        let Some(store) = test_store().await else {
            return;
        };
        let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let account = format!("acct-order-life-{suffix}");
        let execution = format!("exec-order-life-{suffix}");
        seed_execution_plan(&store, &account, &execution).await;
        let order_id = format!("order-life-{suffix}");
        store
            .upsert_order_lifecycle(&OrderLifecycleRecord {
                order_id: order_id.clone(),
                execution_id: execution,
                account_id: account,
                condition_id: "cond-order-life".into(),
                token_id: "token-order-life".into(),
                side: "BUY".into(),
                lifecycle_state: OrderLifecycleState::Posted,
                remote_order_id: Some(format!("remote-{order_id}")),
                remote_state: Some("OPEN".into()),
                created_at: None,
                updated_at: None,
            })
            .await
            .expect("upsert order");
        let updated = store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: order_id.clone(),
                event: OrderEventKind::CancelRequested,
                event_source: "pmx-store-test".into(),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record order event");
        assert_eq!(
            updated.lifecycle_state,
            OrderLifecycleState::CancelRequested
        );
        let events = store
            .list_order_lifecycle_events(&OrderLifecycleEventQuery {
                order_id,
                limit: 10,
                before_event_id: None,
            })
            .await
            .expect("list order events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, OrderEventKind::CancelRequested);
    }
}
