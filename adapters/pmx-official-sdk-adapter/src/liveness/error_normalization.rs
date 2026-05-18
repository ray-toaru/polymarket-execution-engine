use crate::{OfficialSdkErrorCategory, OfficialSdkNormalizedError};
use polymarket_client_sdk_v2::error::{
    Error as SdkError, Geoblock as SdkGeoblock, Kind as SdkErrorKind, Status as SdkStatus,
};

pub fn normalize_sdk_error(error: &SdkError) -> OfficialSdkNormalizedError {
    match error.kind() {
        SdkErrorKind::Validation => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::ValidationFailed,
            retryable: false,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Synchronization => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Geoblock => {
            let geoblock = error.downcast_ref::<SdkGeoblock>();
            OfficialSdkNormalizedError {
                category: OfficialSdkErrorCategory::Geoblocked,
                retryable: false,
                message: error.to_string(),
                http_status: None,
                geoblock_country: geoblock.map(|g| g.country.clone()),
                geoblock_region: geoblock.map(|g| g.region.clone()),
            }
        }
        SdkErrorKind::WebSocket => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::WebSocketFailed,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        SdkErrorKind::Status => {
            let status = error.downcast_ref::<SdkStatus>();
            let code = status.map(|s| s.status_code.as_u16());
            let category = match code {
                Some(401 | 403) => OfficialSdkErrorCategory::AuthenticationFailed,
                Some(408 | 429 | 500..=599) => OfficialSdkErrorCategory::RemoteUnknown,
                _ => OfficialSdkErrorCategory::RemoteRejected,
            };
            let retryable = matches!(code, Some(408 | 429 | 500..=599));
            OfficialSdkNormalizedError {
                category,
                retryable,
                message: error.to_string(),
                http_status: code,
                geoblock_country: None,
                geoblock_region: None,
            }
        }
        SdkErrorKind::Internal => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
        _ => OfficialSdkNormalizedError {
            category: OfficialSdkErrorCategory::Internal,
            retryable: true,
            message: error.to_string(),
            http_status: None,
            geoblock_country: None,
            geoblock_region: None,
        },
    }
}
