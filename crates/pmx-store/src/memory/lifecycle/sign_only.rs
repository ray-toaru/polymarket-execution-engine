use super::*;

pub fn record_sign_only_lifecycle_event(
    store: &InMemoryStore,
    record: &SignOnlyLifecycleRecord,
) -> Result<(), StoreError> {
    let mut state = store.inner.lock().expect("in-memory store mutex poisoned");
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

pub fn list_sign_only_lifecycle_events(
    store: &InMemoryStore,
    query: &SignOnlyLifecycleQuery,
) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
    let mut records: Vec<_> = store
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
