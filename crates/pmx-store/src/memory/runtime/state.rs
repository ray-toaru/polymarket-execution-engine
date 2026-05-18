use async_trait::async_trait;
use pmx_core::{CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus};

use super::super::InMemoryStore;
use crate::{
    RuntimeStateQuery, RuntimeStateStore, StoreError, apply_runtime_worker_observations,
    worker_status_from_heartbeats,
};

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
