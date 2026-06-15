use super::*;

pub fn upsert_order_lifecycle(
    store: &InMemoryStore,
    order: &OrderLifecycleRecord,
) -> Result<(), StoreError> {
    let mut stored = order.clone();
    let now = Utc::now();
    if stored.created_at.is_none() {
        stored.created_at = Some(now);
    }
    stored.updated_at = Some(now);
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .orders
        .insert(stored.order_id.clone(), stored);
    Ok(())
}

pub fn record_order_lifecycle_event(
    store: &InMemoryStore,
    event: &OrderLifecycleEventRecord,
) -> Result<OrderLifecycleRecord, StoreError> {
    let mut state = store.inner.lock().expect("in-memory store mutex poisoned");
    let Some(current) = state.orders.get(&event.order_id).cloned() else {
        return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
    };
    if let Some(correlation_id) = event.correlation_id.as_deref()
        && let Some(previous) = state.order_events.iter().find(|candidate| {
            candidate.order_id == event.order_id
                && candidate.correlation_id.as_deref() == Some(correlation_id)
        })
    {
        if previous.event == event.event
            && previous.event_source == event.event_source
            && previous.payload == event.payload
        {
            return Ok(current);
        }
        return Err(StoreError::Conflict(
            "order lifecycle correlation_id reused with different event payload".into(),
        ));
    }
    let order = state
        .orders
        .get_mut(&event.order_id)
        .expect("order existence checked above");
    let next = transition_order_state(order.lifecycle_state.clone(), event.event.clone())
        .map_err(|err| StoreError::Conflict(err.to_string()))?;
    order.lifecycle_state = next;
    order.updated_at = Some(Utc::now());
    let updated = order.clone();
    state.order_event_counter += 1;
    let mut stored_event = event.clone();
    stored_event.event_id = Some(state.order_event_counter);
    stored_event.created_at = Some(Utc::now());
    state.order_events.push(stored_event);
    Ok(updated)
}

pub fn load_order_lifecycle(
    store: &InMemoryStore,
    order_id: &str,
) -> Result<Option<OrderLifecycleRecord>, StoreError> {
    Ok(store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .orders
        .get(order_id)
        .cloned())
}
