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

#[async_trait]
impl ExecutionLifecycleStore for InMemoryStore {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.lifecycle_event_counter += 1;
        let mut stored = sanitize_execution_lifecycle_event(event.clone());
        stored.event_id = Some(state.lifecycle_event_counter);
        stored.created_at = Some(Utc::now());
        state.lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .lifecycle_events
            .iter()
            .filter(|event| event.execution_id == query.execution_id)
            .filter(|event| {
                query
                    .before_event_id
                    .map(|before| event.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.event_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl SignOnlyLifecycleStore for InMemoryStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        if !state.plans.contains_key(&record.execution_id.0) {
            return Err(StoreError::NotFound(format!(
                "execution_id={}",
                record.execution_id.0
            )));
        }
        let existing: Vec<_> = state
            .sign_only_lifecycle_events
            .iter()
            .filter(|existing| existing.execution_id == record.execution_id)
            .cloned()
            .collect();
        if sign_only_lifecycle_record_is_replay(&existing, record)? {
            return Ok(());
        }
        validate_sign_only_lifecycle_append_for_store(&existing, record)?;
        state.sign_only_event_counter += 1;
        let mut stored = sanitize_sign_only_lifecycle_record(record.clone());
        stored.event_id = Some(state.sign_only_event_counter);
        stored.created_at = Some(Utc::now());
        state.sign_only_lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        let mut records: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .sign_only_lifecycle_events
            .iter()
            .filter(|record| record.execution_id.0 == query.execution_id)
            .filter(|record| {
                query
                    .before_event_id
                    .map(|before| record.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        records.sort_by_key(|record| record.event_id.unwrap_or(0));
        records.reverse();
        records.truncate(query.bounded_limit());
        records.reverse();
        Ok(records)
    }
}
