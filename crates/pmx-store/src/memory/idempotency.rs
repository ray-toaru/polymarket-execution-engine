use super::{IdempotencyRecord, InMemoryStore, attempt_counter_key, identity};
use async_trait::async_trait;

use crate::{IdempotencyAction, IdempotencyStore, StoreError};

#[async_trait]
impl IdempotencyStore for InMemoryStore {
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        if let Some(existing) = state.idempotency.get(&key) {
            if existing.request_fingerprint != request_fingerprint {
                return Ok(IdempotencyAction::Conflict);
            }
            if let (Some(response_fingerprint), Some(response_json)) =
                (&existing.response_fingerprint, &existing.response_json)
            {
                return Ok(IdempotencyAction::ReplayStoredResponse {
                    response_fingerprint: response_fingerprint.clone(),
                    response_json: response_json.clone(),
                });
            }
            return Ok(IdempotencyAction::InProgress {
                submit_attempt: existing.submit_attempt,
                retry_after_ms: 1_000,
            });
        }

        let counter_key = attempt_counter_key(account_id, execution_id);
        let next_attempt = state
            .attempt_counters
            .get(&counter_key)
            .copied()
            .unwrap_or(0)
            + 1;
        state.attempt_counters.insert(counter_key, next_attempt);
        state.idempotency.insert(
            key,
            IdempotencyRecord {
                submit_attempt: next_attempt,
                request_fingerprint: request_fingerprint.into(),
                response_fingerprint: None,
                response_json: None,
            },
        );
        Ok(IdempotencyAction::Proceed {
            submit_attempt: next_attempt,
            owner_token: format!("owner-{account_id}-{execution_id}-{next_attempt}"),
        })
    }

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        let record = state
            .idempotency
            .get_mut(&key)
            .ok_or_else(|| StoreError::NotFound(key.clone()))?;
        if record.request_fingerprint != request_fingerprint {
            return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
        }
        record.response_fingerprint = Some(response_fingerprint.into());
        record.response_json = Some(response_json.into());
        Ok(())
    }
}
