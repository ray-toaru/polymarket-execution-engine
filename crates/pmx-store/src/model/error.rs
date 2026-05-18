use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database unavailable: {0}")]
    DatabaseUnavailable(String),
    #[error("serialization failure; retryable")]
    SerializationFailure,
    #[error("unexpected db data: {0}")]
    InvalidData(String),
}
