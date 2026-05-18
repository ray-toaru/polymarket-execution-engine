use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::super::InMemoryStore;
use crate::{
    RuntimeWorkerStatusQuery, RuntimeWorkerStatusReport, RuntimeWorkerStatusStore, StoreError,
};

#[async_trait]
impl RuntimeWorkerStatusStore for InMemoryStore {
    async fn list_runtime_worker_status(
        &self,
        query: &RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, StoreError> {
        let state = self.inner.lock().expect("in-memory store mutex poisoned");
        let mut heartbeats: Vec<_> = state.worker_health.values().cloned().collect();
        heartbeats.sort_by(|left, right| {
            right
                .last_heartbeat_at
                .cmp(&left.last_heartbeat_at)
                .then_with(|| left.worker_id.cmp(&right.worker_id))
        });
        heartbeats.truncate(query.bounded_limit());

        let mut observations: Vec<_> = state
            .runtime_worker_observations
            .iter()
            .filter(|observation| observation.account_id == query.account_id)
            .filter(|observation| {
                query
                    .before_observed_at
                    .map(|before| {
                        observation.observed_at.unwrap_or(DateTime::<Utc>::MAX_UTC) < before
                    })
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        observations.sort_by(|left, right| {
            right
                .observed_at
                .cmp(&left.observed_at)
                .then_with(|| left.capability.cmp(&right.capability))
        });
        observations.truncate(query.bounded_limit());
        observations.reverse();
        Ok(RuntimeWorkerStatusReport {
            heartbeats,
            observations,
        })
    }
}
