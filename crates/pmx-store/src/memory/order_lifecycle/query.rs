use super::*;

pub fn list_order_lifecycle_events(
    store: &InMemoryStore,
    query: &OrderLifecycleEventQuery,
) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
    let mut events: Vec<_> = store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .order_events
        .iter()
        .filter(|event| event.order_id == query.order_id)
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
