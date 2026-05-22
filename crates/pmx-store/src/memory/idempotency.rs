use super::{IdempotencyRecord, IdempotencyStatus, InMemoryStore, attempt_counter_key, identity};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{FinishSubmitAttempt, IdempotencyAction, IdempotencyStore, StoreError};

const IDEMPOTENCY_LEASE_SECS: i64 = 30;

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
        let now = Utc::now();
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
            if existing.status == IdempotencyStatus::Proceeding && existing.lease_expires_at > now {
                let retry_after_ms = (existing.lease_expires_at - now)
                    .num_milliseconds()
                    .max(1_000) as u64;
                return Ok(IdempotencyAction::InProgress {
                    submit_attempt: existing.submit_attempt,
                    retry_after_ms,
                });
            }
        }

        let counter_key = attempt_counter_key(account_id, execution_id);
        let existing_attempt = state
            .idempotency
            .get(&key)
            .map(|record| record.submit_attempt)
            .unwrap_or(0);
        let next_attempt = state
            .attempt_counters
            .get(&counter_key)
            .copied()
            .unwrap_or(existing_attempt)
            .max(existing_attempt)
            + 1;
        state.attempt_counters.insert(counter_key, next_attempt);
        let owner_token = format!("owner-{}", Uuid::new_v4());
        if let Some(existing) = state.idempotency.get_mut(&key) {
            existing.submit_attempt = next_attempt;
            existing.status = IdempotencyStatus::Proceeding;
            existing.owner_token = owner_token.clone();
            existing.lease_expires_at = now + Duration::seconds(IDEMPOTENCY_LEASE_SECS);
            existing.response_fingerprint = None;
            existing.response_json = None;
            return Ok(IdempotencyAction::Proceed {
                submit_attempt: next_attempt,
                owner_token,
            });
        }
        state.idempotency.insert(
            key,
            IdempotencyRecord {
                submit_attempt: next_attempt,
                request_fingerprint: request_fingerprint.into(),
                status: IdempotencyStatus::Proceeding,
                owner_token: owner_token.clone(),
                lease_expires_at: now + Duration::seconds(IDEMPOTENCY_LEASE_SECS),
                response_fingerprint: None,
                response_json: None,
            },
        );
        Ok(IdempotencyAction::Proceed {
            submit_attempt: next_attempt,
            owner_token,
        })
    }

    async fn finish_submit_attempt(
        &self,
        attempt: FinishSubmitAttempt<'_>,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(
            attempt.account_id,
            attempt.execution_id,
            attempt.idempotency_key,
        );
        let record = state
            .idempotency
            .get_mut(&key)
            .ok_or_else(|| StoreError::NotFound(key.clone()))?;
        if record.request_fingerprint != attempt.request_fingerprint {
            return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
        }
        if record.status != IdempotencyStatus::Proceeding
            || record.owner_token != attempt.owner_token
        {
            return Err(StoreError::Conflict(
                "idempotency owner_token does not own proceeding attempt".into(),
            ));
        }
        record.status = IdempotencyStatus::Done;
        record.response_fingerprint = Some(attempt.response_fingerprint.into());
        record.response_json = Some(attempt.response_json.into());
        Ok(())
    }
}
