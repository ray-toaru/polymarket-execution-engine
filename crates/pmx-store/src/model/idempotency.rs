use async_trait::async_trait;

use super::StoreError;

#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Begin or replay a submit request.
    ///
    /// Canonical identity is `(account_id, execution_id, idempotency_key)`.
    /// `submit_attempt` is executor-generated inside the transaction and is not supplied by the control plane.
    /// A different request fingerprint under the same identity must return `Conflict`.
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError>;

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyAction {
    /// This caller owns the in-progress side-effect slot and may continue.
    Proceed {
        submit_attempt: u32,
        owner_token: String,
    },
    /// Another caller already owns this idempotency identity and has not finished.
    /// Retrying callers must not sign/post remotely while this is fresh.
    InProgress {
        submit_attempt: u32,
        retry_after_ms: u64,
    },
    ReplayStoredResponse {
        response_fingerprint: String,
        response_json: String,
    },
    Conflict,
}
