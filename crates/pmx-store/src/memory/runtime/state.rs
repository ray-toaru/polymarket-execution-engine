use async_trait::async_trait;
use chrono::Utc;
use pmx_core::{
    CollateralProfileStatus, GeoblockStatus, KillSwitchScope, RuntimeStateSummary, WorkerStatus,
};

use super::super::InMemoryStore;
use crate::{
    KillSwitchStateChange, RuntimeControlStore, RuntimeStateQuery, RuntimeStateStore, StoreError,
    apply_runtime_worker_observations, worker_status_from_heartbeats,
};

#[async_trait]
impl RuntimeStateStore for InMemoryStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let mut base =
            {
                let state = self.inner.lock().expect("in-memory store mutex poisoned");
                let mut base = state.runtime_states.get(&query.key()).cloned().unwrap_or(
                    RuntimeStateSummary {
                        geoblock_status: GeoblockStatus::Unknown,
                        worker_status: WorkerStatus::Unknown,
                        collateral_profile_status: CollateralProfileStatus::Unknown,
                        kill_switch_enabled: true,
                        required_capabilities: query.required_capabilities.clone(),
                    },
                );
                if let Some(kill_switch) = state.account_kill_switches.get(&query.account_id) {
                    base.kill_switch_enabled = kill_switch.enabled;
                }
                if state
                    .global_kill_switch
                    .as_ref()
                    .map(|kill_switch| kill_switch.enabled)
                    .unwrap_or(false)
                {
                    base.kill_switch_enabled = true;
                }
                base
            };
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

#[async_trait]
impl RuntimeControlStore for InMemoryStore {
    async fn set_account_kill_switch(
        &self,
        account_id: &pmx_core::AccountId,
        enabled: bool,
        _reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError> {
        let effective_at = Utc::now();
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let next_version = state
            .account_kill_switches
            .get(&account_id.0)
            .map(|existing| existing.state_version + 1)
            .unwrap_or(1);
        state.account_kill_switches.insert(
            account_id.0.clone(),
            super::super::AccountKillSwitchState {
                enabled,
                state_version: next_version,
            },
        );
        Ok(KillSwitchStateChange {
            scope: KillSwitchScope::Account,
            account_id: Some(account_id.clone()),
            enabled,
            state_version: next_version,
            effective_at,
        })
    }

    async fn set_global_kill_switch(
        &self,
        enabled: bool,
        _reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError> {
        let effective_at = Utc::now();
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let next_version = state
            .global_kill_switch
            .as_ref()
            .map(|existing| existing.state_version + 1)
            .unwrap_or(1);
        state.global_kill_switch = Some(super::super::AccountKillSwitchState {
            enabled,
            state_version: next_version,
        });
        Ok(KillSwitchStateChange {
            scope: KillSwitchScope::Global,
            account_id: None,
            enabled,
            state_version: next_version,
            effective_at,
        })
    }
}
