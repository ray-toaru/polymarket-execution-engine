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
        attempt: FinishSubmitAttempt<'_>,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinishSubmitAttempt<'a> {
    pub account_id: &'a str,
    pub execution_id: &'a str,
    pub idempotency_key: &'a str,
    pub request_fingerprint: &'a str,
    pub owner_token: &'a str,
    pub response_fingerprint: &'a str,
    pub response_json: &'a str,
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
