use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("remote rejected request: {0}")]
    RemoteRejected(String),
    #[error("remote state unknown: {0}")]
    RemoteUnknown(String),
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("signing unavailable")]
    SigningUnavailable,
    #[error("gateway is intentionally disabled in scaffold mode")]
    Disabled,
}
