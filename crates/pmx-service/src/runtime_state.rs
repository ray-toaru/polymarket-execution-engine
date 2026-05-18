use async_trait::async_trait;
use pmx_core::{
    CollateralProfileStatus, GeoblockStatus, NormalizedIntent, RuntimeStateSummary, WorkerStatus,
};
use pmx_store::{RuntimeStateQuery, RuntimeStateStore};

#[async_trait]
pub trait RuntimeStateProvider: Clone + Send + Sync + 'static {
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary;
}

fn fail_closed_runtime_state(required_capabilities: Vec<String>) -> RuntimeStateSummary {
    RuntimeStateSummary {
        geoblock_status: GeoblockStatus::Unknown,
        worker_status: WorkerStatus::Unknown,
        collateral_profile_status: CollateralProfileStatus::Unknown,
        kill_switch_enabled: true,
        required_capabilities,
    }
}

#[derive(Debug, Clone, Default)]
pub struct FailClosedRuntimeStateProvider;

#[async_trait]
impl RuntimeStateProvider for FailClosedRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        fail_closed_runtime_state(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct StaticRuntimeStateProvider {
    runtime_state: RuntimeStateSummary,
}

impl StaticRuntimeStateProvider {
    pub fn new(runtime_state: RuntimeStateSummary) -> Self {
        Self { runtime_state }
    }
}

#[async_trait]
impl RuntimeStateProvider for StaticRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        self.runtime_state.clone()
    }
}

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
