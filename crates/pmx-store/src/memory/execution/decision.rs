use super::*;

pub fn save_decision(
    store: &InMemoryStore,
    decision: &ConstraintDecision,
) -> Result<(), StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .decisions
        .insert(decision.decision_id.clone(), decision.clone());
    Ok(())
}

pub fn load_decision(
    store: &InMemoryStore,
    decision_id: &str,
) -> Result<ConstraintDecision, StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .decisions
        .get(decision_id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(format!("decision_id={decision_id}")))
}
