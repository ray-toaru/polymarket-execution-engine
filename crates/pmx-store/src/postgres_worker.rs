use async_trait::async_trait;
use chrono::Utc;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore, RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport,
    RuntimeWorkerStatusStore, StoreError,
};

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
