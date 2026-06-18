use super::InMemoryStore;
use async_trait::async_trait;
use chrono::Utc;

use crate::{
    LiveReadEventQuery, LiveReadEventRecord, LiveReadEventStore, StoreError,
    sanitize_live_read_event, validate_live_read_event_for_store,
};

#[async_trait]
impl LiveReadEventStore for InMemoryStore {
    async fn record_live_read_event(&self, event: &LiveReadEventRecord) -> Result<(), StoreError> {
        validate_live_read_event_for_store(event)?;
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.live_read_event_counter += 1;
        let mut stored = sanitize_live_read_event(event.clone());
        stored.event_id = Some(state.live_read_event_counter);
        stored.observed_at = Some(Utc::now());
        state.live_read_events.push(stored);
        Ok(())
    }

    async fn list_live_read_events(
        &self,
        query: &LiveReadEventQuery,
    ) -> Result<Vec<LiveReadEventRecord>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .live_read_events
            .iter()
            .filter(|event| {
                query
                    .before_event_id
                    .map(|before| event.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .account_id
                    .as_ref()
                    .map(|account_id| &event.account_id == account_id)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .operation
                    .as_ref()
                    .map(|operation| &event.operation == operation)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .outcome
                    .as_ref()
                    .map(|outcome| &event.outcome == outcome)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .remote_order_id
                    .as_ref()
                    .map(|remote_order_id| event.remote_order_id.as_ref() == Some(remote_order_id))
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
