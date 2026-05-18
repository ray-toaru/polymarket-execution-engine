use pmx_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("in progress: retry_after_ms={retry_after_ms}")]
    InProgress { retry_after_ms: u64 },
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("internal error: {0}")]
    Internal(String),
}
