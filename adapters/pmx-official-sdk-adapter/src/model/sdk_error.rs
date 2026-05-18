use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialSdkErrorCategory {
    RemoteRejected,
    RemoteUnknown,
    AuthenticationFailed,
    ValidationFailed,
    Geoblocked,
    WebSocketFailed,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkNormalizedError {
    pub category: OfficialSdkErrorCategory,
    pub retryable: bool,
    pub message: String,
    pub http_status: Option<u16>,
    pub geoblock_country: Option<String>,
    pub geoblock_region: Option<String>,
}
