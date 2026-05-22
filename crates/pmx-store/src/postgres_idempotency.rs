use async_trait::async_trait;

mod begin;
mod finish;

use crate::postgres::PostgresStore;
use crate::{FinishSubmitAttempt, IdempotencyAction, IdempotencyStore, StoreError};

#[async_trait]
impl IdempotencyStore for PostgresStore {
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError> {
        begin::begin_submit_attempt(
            self,
            account_id,
            execution_id,
            idempotency_key,
            request_fingerprint,
        )
        .await
    }

    async fn finish_submit_attempt(
        &self,
        attempt: FinishSubmitAttempt<'_>,
    ) -> Result<(), StoreError> {
        finish::finish_submit_attempt(self, attempt).await
    }
}
