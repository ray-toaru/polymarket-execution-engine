mod audit;
mod flow;
mod lifecycle;
mod runtime;
mod sign_only;
mod submit;

use pmx_service::{ExecutorService, StoreBackedRuntimeStateProvider};
use pmx_store::{InMemoryStore, PostgresStore};

pub(crate) const CONTRACT_VERSION: &str = "1.0.0-draft";

#[derive(Clone)]
pub enum ServiceBackend {
    InMemory(ExecutorService<InMemoryStore>),
    Postgres(ExecutorService<PostgresStore, StoreBackedRuntimeStateProvider<PostgresStore>>),
}

impl ServiceBackend {
    pub(crate) fn storage_mode(&self) -> &'static str {
        match self {
            Self::InMemory(_) => "in_memory_scaffold",
            Self::Postgres(_) => "postgres",
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) service: ServiceBackend,
}

impl AppState {
    pub fn in_memory() -> Self {
        Self {
            service: ServiceBackend::InMemory(ExecutorService::new(InMemoryStore::default())),
        }
    }

    pub fn in_memory_with_store(store: InMemoryStore) -> Self {
        Self {
            service: ServiceBackend::InMemory(ExecutorService::new(store)),
        }
    }

    pub fn postgres(store: PostgresStore) -> Self {
        let provider = StoreBackedRuntimeStateProvider::new(store.clone());
        Self {
            service: ServiceBackend::Postgres(ExecutorService::with_runtime_provider(
                store,
                provider,
                env!("CARGO_PKG_VERSION").to_owned(),
                CONTRACT_VERSION.to_owned(),
            )),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::in_memory()
    }
}
