use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OfficialSdkAdapterError {
    #[error("operation disabled by adapter safety gate: {0}")]
    SafetyGate(String),
    #[error("required credential or environment value is missing: {0}")]
    MissingCredential(String),
    #[error("input is invalid for official SDK mapping: {0}")]
    InvalidInput(String),
    #[error("official SDK operation failed: {0}")]
    OperationFailed(String),
    #[error("SDK dependency is not enabled for this build")]
    SdkFeatureDisabled,
}
