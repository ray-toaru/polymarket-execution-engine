use super::*;

pub fn save_normalized_intent(
    store: &InMemoryStore,
    intent: &NormalizedIntent,
) -> Result<(), StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .normalized
        .insert(intent.normalized_intent_id.clone(), intent.clone());
    Ok(())
}

pub fn load_normalized_intent(
    store: &InMemoryStore,
    normalized_intent_id: &str,
) -> Result<NormalizedIntent, StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .normalized
        .get(normalized_intent_id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(format!("normalized_intent_id={normalized_intent_id}")))
}

pub fn save_snapshot(
    store: &InMemoryStore,
    snapshot: &FeasibilitySnapshot,
) -> Result<(), StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .snapshots
        .insert(snapshot.snapshot_id.clone(), snapshot.clone());
    Ok(())
}

pub fn load_snapshot(
    store: &InMemoryStore,
    snapshot_id: &str,
) -> Result<FeasibilitySnapshot, StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .snapshots
        .get(snapshot_id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(format!("snapshot_id={snapshot_id}")))
}
