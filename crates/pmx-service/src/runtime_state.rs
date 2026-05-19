use async_trait::async_trait;
use pmx_core::{
    CollateralProfileStatus, GeoblockStatus, NormalizedIntent, RuntimeStateSummary, WorkerStatus,
};
use pmx_store::{RuntimeStateQuery, RuntimeStateStore};

#[path = "runtime_state/fail_closed.rs"]
mod fail_closed;

#[path = "runtime_state/static_provider.rs"]
mod static_provider;

#[path = "runtime_state/store_backed.rs"]
mod store_backed;

pub use fail_closed::{FailClosedRuntimeStateProvider, fail_closed_runtime_state};
pub use static_provider::StaticRuntimeStateProvider;
pub use store_backed::StoreBackedRuntimeStateProvider;

#[async_trait]
pub trait RuntimeStateProvider: Clone + Send + Sync + 'static {
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary;
}
