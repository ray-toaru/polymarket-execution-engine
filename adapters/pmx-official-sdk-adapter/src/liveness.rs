use crate::{OfficialSdkLivenessSnapshot, OfficialSdkReconcileDisposition};
use pmx_core::GeoblockStatus;

#[cfg(not(feature = "sdk-typecheck"))]
use crate::OfficialSdkAdapterError;
#[cfg(feature = "sdk-typecheck")]
use crate::{OfficialSdkErrorCategory, OfficialSdkNormalizedError};
#[cfg(feature = "sdk-typecheck")]
use polymarket_client_sdk_v2::error::{
    Error as SdkError, Geoblock as SdkGeoblock, Kind as SdkErrorKind, Status as SdkStatus,
};

pub fn assess_sdk_liveness(
    snapshot: &OfficialSdkLivenessSnapshot,
) -> OfficialSdkReconcileDisposition {
    if snapshot.geoblock_status == GeoblockStatus::Blocked {
        return OfficialSdkReconcileDisposition::Geoblocked;
    }
    if !snapshot.websocket_connected || (snapshot.heartbeat_expected && !snapshot.heartbeats_active)
    {
        return OfficialSdkReconcileDisposition::ReconnectWebsocket;
    }
    if snapshot.remote_unknown_orders > 0 {
        return OfficialSdkReconcileDisposition::ReconcileRequired;
    }
    OfficialSdkReconcileDisposition::Healthy
}

#[cfg(feature = "sdk-typecheck")]
pub fn sdk_type_markers() -> Vec<&'static str> {
    vec![
        std::any::type_name::<polymarket_client_sdk_v2::clob::Client>(),
        std::any::type_name::<polymarket_client_sdk_v2::clob::Config>(),
        std::any::type_name::<polymarket_client_sdk_v2::types::Decimal>(),
    ]
}

#[cfg(not(feature = "sdk-typecheck"))]
pub fn sdk_type_markers() -> Result<(), OfficialSdkAdapterError> {
    Err(OfficialSdkAdapterError::SdkFeatureDisabled)
}

#[cfg(feature = "sdk-typecheck")]
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

#[cfg(feature = "sdk-typecheck")]
pub fn geoblock_status_from_sdk(blocked: bool) -> GeoblockStatus {
    if blocked {
        GeoblockStatus::Blocked
    } else {
        GeoblockStatus::Allowed
    }
}
