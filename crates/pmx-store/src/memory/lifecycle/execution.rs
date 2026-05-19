use super::*;

pub fn record_execution_lifecycle_event(
    store: &InMemoryStore,
    event: &ExecutionLifecycleEvent,
) -> Result<(), StoreError> {
    let mut state = store.inner.lock().expect("in-memory store mutex poisoned");
    state.lifecycle_event_counter += 1;
    let mut stored = sanitize_execution_lifecycle_event(event.clone());
    stored.event_id = Some(state.lifecycle_event_counter);
    stored.created_at = Some(Utc::now());
    state.lifecycle_events.push(stored);
    Ok(())
}

pub fn list_execution_lifecycle_events(
    store: &InMemoryStore,
    query: &ExecutionLifecycleQuery,
) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
    let mut events: Vec<_> = store
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
