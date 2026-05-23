use super::*;
use crate::ServiceError;

#[derive(Debug, Clone)]
pub struct StoreBackedRuntimeStateProvider<S> {
    store: S,
    required_capabilities: Vec<String>,
}

impl<S> StoreBackedRuntimeStateProvider<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
        }
    }

    pub fn with_required_capabilities(store: S, required_capabilities: Vec<String>) -> Self {
        Self {
            store,
            required_capabilities,
        }
    }

    pub async fn load_canary_runtime_truth(
        &self,
        query: &pmx_store::CanaryRuntimeTruthQuery,
    ) -> Result<pmx_store::CanaryRuntimeTruthBindings, ServiceError>
    where
        S: pmx_store::CanaryRuntimeTruthStore,
    {
        self.store
            .load_canary_runtime_truth(query)
            .await
            .map_err(ServiceError::Store)
    }
}

#[async_trait]
impl<S> RuntimeStateProvider for StoreBackedRuntimeStateProvider<S>
where
    S: RuntimeStateStore + Clone + Send + Sync + 'static,
{
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        let query = RuntimeStateQuery {
            account_id: normalized_intent.account_id.0.clone(),
            condition_id: normalized_intent.market.condition_id.0.clone(),
            collateral_profile_id: normalized_intent.collateral_profile_id.clone(),
            required_capabilities: self.required_capabilities.clone(),
        };
        self.store
            .load_runtime_state(&query)
            .await
            .unwrap_or_else(|_| fail_closed_runtime_state(query.required_capabilities))
    }
}
