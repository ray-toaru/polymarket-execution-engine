use super::InMemoryStore;
use async_trait::async_trait;
use pmx_core::AccountId;

use crate::{
    RealFundsCanaryLifecycleState, RealFundsCanaryRunRecord, RealFundsCanaryRunStore, StoreError,
    validate_real_funds_canary_transition,
};

#[async_trait]
impl RealFundsCanaryRunStore for InMemoryStore {
    async fn record_real_funds_canary_run(
        &self,
        record: &RealFundsCanaryRunRecord,
    ) -> Result<RealFundsCanaryRunRecord, StoreError> {
        validate_record(record)?;
        let key = idempotency_key(&record.account_id, &record.idempotency_key);
        let mut state = self.inner.lock().expect("in-memory store lock");
        if let Some(run_id) = state.real_funds_canary_idempotency.get(&key) {
            let existing = state
                .real_funds_canary_runs
                .get(run_id)
                .ok_or_else(|| StoreError::InvalidData("missing canary run index target".into()))?;
            if existing.same_idempotent_request(record) {
                return Ok(existing.clone());
            }
            return Err(StoreError::Conflict(
                "real-funds canary idempotency key reused with different request".into(),
            ));
        }
        if state.real_funds_canary_runs.contains_key(&record.run_id) {
            return Err(StoreError::Conflict(format!(
                "real-funds canary run already exists: {}",
                record.run_id
            )));
        }
        state
            .real_funds_canary_idempotency
            .insert(key, record.run_id.clone());
        state
            .real_funds_canary_runs
            .insert(record.run_id.clone(), record.clone());
        Ok(record.clone())
    }

    async fn load_real_funds_canary_run(
        &self,
        run_id: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError> {
        let state = self.inner.lock().expect("in-memory store lock");
        Ok(state.real_funds_canary_runs.get(run_id).cloned())
    }

    async fn load_real_funds_canary_run_by_idempotency(
        &self,
        account_id: &AccountId,
        idempotency: &str,
    ) -> Result<Option<RealFundsCanaryRunRecord>, StoreError> {
        let key = idempotency_key(account_id, idempotency);
        let state = self.inner.lock().expect("in-memory store lock");
        Ok(state
            .real_funds_canary_idempotency
            .get(&key)
            .and_then(|run_id| state.real_funds_canary_runs.get(run_id))
            .cloned())
    }

    async fn update_real_funds_canary_state(
        &self,
        run_id: &str,
        lifecycle_state: RealFundsCanaryLifecycleState,
        remote_status: Option<String>,
    ) -> Result<RealFundsCanaryRunRecord, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store lock");
        let record = state
            .real_funds_canary_runs
            .get_mut(run_id)
            .ok_or_else(|| StoreError::NotFound(format!("real-funds canary run: {run_id}")))?;
        validate_real_funds_canary_transition(&record.lifecycle_state, &lifecycle_state)?;
        record.lifecycle_state = lifecycle_state;
        record.remote_status = remote_status;
        Ok(record.clone())
    }
}

fn idempotency_key(account_id: &AccountId, idempotency_key: &str) -> String {
    format!("{}\u{1f}{idempotency_key}", account_id.0)
}

fn validate_record(record: &RealFundsCanaryRunRecord) -> Result<(), StoreError> {
    if record.raw_signed_order_exposed {
        return Err(StoreError::Conflict(
            "real-funds canary must not expose raw signed order material".into(),
        ));
    }
    if record.remote_side_effects {
        return Err(StoreError::Conflict(
            "local real-funds canary lifecycle records must not mark remote side effects".into(),
        ));
    }
    Ok(())
}
