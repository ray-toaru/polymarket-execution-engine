use super::InMemoryStore;
use async_trait::async_trait;
use chrono::Utc;
use pmx_core::SignOnlyLifecycleRecord;

use crate::{
    ExecutionLifecycleEvent, ExecutionLifecycleQuery, ExecutionLifecycleStore,
    SignOnlyLifecycleQuery, SignOnlyLifecycleStore, StoreError, sanitize_execution_lifecycle_event,
    sanitize_sign_only_lifecycle_record, sign_only_lifecycle_record_is_replay,
    validate_sign_only_lifecycle_append_for_store,
};

#[path = "lifecycle/execution.rs"]
mod execution;

#[path = "lifecycle/sign_only.rs"]
mod sign_only;

#[async_trait]
impl ExecutionLifecycleStore for InMemoryStore {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError> {
        execution::record_execution_lifecycle_event(self, event)
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
        execution::list_execution_lifecycle_events(self, query)
    }
}

#[async_trait]
impl SignOnlyLifecycleStore for InMemoryStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        sign_only::record_sign_only_lifecycle_event(self, record)
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        sign_only::list_sign_only_lifecycle_events(self, query)
    }
}
