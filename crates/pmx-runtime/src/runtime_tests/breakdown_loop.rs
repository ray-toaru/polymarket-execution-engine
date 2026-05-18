use super::*;

struct FakeRuntimeWorkerProvider(RuntimeWorkerProviderSnapshot);

impl RuntimeWorkerProvider for FakeRuntimeWorkerProvider {
    fn snapshot(&self) -> RuntimeWorkerProviderSnapshot {
        self.0.clone()
    }
}

#[path = "breakdown_loop/capabilities.rs"]
mod capabilities;

#[path = "breakdown_loop/worker_loop.rs"]
mod worker_loop;

#[path = "breakdown_loop/provider.rs"]
mod provider;
