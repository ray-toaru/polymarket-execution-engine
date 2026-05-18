use super::InMemoryStore;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus};

use crate::{
    RuntimeStateQuery, RuntimeStateStore, RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat,
    RuntimeWorkerObservation, RuntimeWorkerObservationStore, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusReport, RuntimeWorkerStatusStore, StoreError,
    apply_runtime_worker_observations, runtime_observation_is_fresh, worker_status_from_heartbeats,
};

impl InMemoryStore {
    pub fn set_runtime_state_for_test(
        &self,
        account_id: &str,
        condition_id: &str,
        collateral_profile_id: Option<&str>,
        runtime_state: RuntimeStateSummary,
    ) {
        let query = RuntimeStateQuery {
            account_id: account_id.to_owned(),
            condition_id: condition_id.to_owned(),
            collateral_profile_id: collateral_profile_id.map(ToOwned::to_owned),
            required_capabilities: vec![],
        };
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .insert(query.key(), runtime_state);
    }

    fn observations_for_account(&self, account_id: &str) -> Vec<RuntimeWorkerObservation> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .iter()
            .filter(|observation| {
                observation.account_id == account_id && runtime_observation_is_fresh(observation)
            })
            .cloned()
            .collect()
    }
}

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

#[async_trait]
impl RuntimeStateStore for InMemoryStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let mut base = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .get(&query.key())
            .cloned()
            .unwrap_or(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Unknown,
                worker_status: WorkerStatus::Unknown,
                collateral_profile_status: CollateralProfileStatus::Unknown,
                kill_switch_enabled: true,
                required_capabilities: query.required_capabilities.clone(),
            });
        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = base.required_capabilities.clone();
        }
        let heartbeats: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .values()
            .cloned()
            .collect();
        if !required_capabilities.is_empty() {
            base.worker_status = worker_status_from_heartbeats(&heartbeats, &required_capabilities);
            base.required_capabilities = required_capabilities;
        }
        Ok(apply_runtime_worker_observations(
            base,
            &self.observations_for_account(&query.account_id),
        ))
    }
}
