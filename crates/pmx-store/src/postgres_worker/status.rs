use async_trait::async_trait;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    RuntimeWorkerHeartbeat, RuntimeWorkerObservation, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusReport, RuntimeWorkerStatusStore, StoreError,
};

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
