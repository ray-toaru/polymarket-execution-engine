use pmx_core::RuntimeStateSummary;

use super::super::InMemoryStore;
use crate::{RuntimeStateQuery, RuntimeWorkerObservation, runtime_observation_is_fresh};

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
            .insert(query.state_scope_key(), runtime_state);
    }

    pub(crate) fn observations_for_account(
        &self,
        account_id: &str,
    ) -> Vec<RuntimeWorkerObservation> {
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
