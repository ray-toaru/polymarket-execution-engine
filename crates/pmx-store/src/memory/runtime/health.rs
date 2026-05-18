use async_trait::async_trait;
use chrono::Utc;

use super::super::InMemoryStore;
use crate::{
    RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat, RuntimeWorkerObservation,
    RuntimeWorkerObservationStore, StoreError,
};

#[async_trait]
impl RuntimeWorkerObservationStore for InMemoryStore {
    async fn record_runtime_worker_observation(
        &self,
        observation: &RuntimeWorkerObservation,
    ) -> Result<(), StoreError> {
        let mut stored = observation.clone();
        if stored.observed_at.is_none() {
            stored.observed_at = Some(Utc::now());
        }
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .push(stored);
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerHealthStore for InMemoryStore {
    async fn record_worker_heartbeat(
        &self,
        heartbeat: &RuntimeWorkerHeartbeat,
    ) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .insert(heartbeat.worker_id.clone(), heartbeat.clone());
        Ok(())
    }
}
