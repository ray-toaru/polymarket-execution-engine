use super::*;

pub fn save_plan_summary(
    store: &InMemoryStore,
    plan: &ExecutionPlanSummary,
) -> Result<(), StoreError> {
    let mut state = store.inner.lock().expect("in-memory store mutex poisoned");
    if let Some(existing) = state.plans.get(&plan.execution_id) {
        if existing == plan {
            return Ok(());
        }
        return Err(StoreError::Conflict(
            "execution plan is immutable and cannot be overwritten".into(),
        ));
    }
    state.plans.insert(plan.execution_id.clone(), plan.clone());
    Ok(())
}

pub fn load_plan_summary(
    store: &InMemoryStore,
    execution_id: &str,
) -> Result<ExecutionPlanSummary, StoreError> {
    store
        .inner
        .lock()
        .expect("in-memory store mutex poisoned")
        .plans
        .get(execution_id)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
}
