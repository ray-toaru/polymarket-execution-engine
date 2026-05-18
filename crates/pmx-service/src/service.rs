mod audit;
mod flow;
mod order_lifecycle;
mod sign_only;
mod submit;

#[cfg(test)]
use pmx_core::*;
#[cfg(test)]
use pmx_store::*;

use pmx_store::{
    AdminAuditStore, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleStore, RuntimeWorkerStatusStore, SignOnlyLifecycleStore,
};

use crate::model::*;
use crate::runtime_state::{FailClosedRuntimeStateProvider, RuntimeStateProvider};

#[derive(Debug, Clone)]
pub struct ExecutorService<S, R = FailClosedRuntimeStateProvider> {
    store: S,
    runtime_state_provider: R,
    executor_version: String,
    contract_version: String,
}

impl<S> ExecutorService<S, FailClosedRuntimeStateProvider>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + RuntimeWorkerStatusStore
        + SignOnlyLifecycleStore
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(store: S) -> Self {
        Self::with_runtime_provider(
            store,
            FailClosedRuntimeStateProvider,
            env!("CARGO_PKG_VERSION").to_owned(),
            DEFAULT_CONTRACT_VERSION.to_owned(),
        )
    }
}

impl<S, R> ExecutorService<S, R>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + RuntimeWorkerStatusStore
        + SignOnlyLifecycleStore
        + Clone
        + Send
        + Sync
        + 'static,
    R: RuntimeStateProvider,
{
    pub fn with_runtime_provider(
        store: S,
        runtime_state_provider: R,
        executor_version: String,
        contract_version: String,
    ) -> Self {
        Self {
            store,
            runtime_state_provider,
            executor_version,
            contract_version,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
