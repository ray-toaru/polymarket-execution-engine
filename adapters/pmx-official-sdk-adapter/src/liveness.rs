#[cfg(feature = "sdk-typecheck")]
mod error_normalization;

use crate::{OfficialSdkLivenessSnapshot, OfficialSdkReconcileDisposition};
use pmx_core::GeoblockStatus;

#[cfg(not(feature = "sdk-typecheck"))]
use crate::OfficialSdkAdapterError;
#[cfg(feature = "sdk-typecheck")]
pub use error_normalization::normalize_sdk_error;

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
pub fn geoblock_status_from_sdk(blocked: bool) -> GeoblockStatus {
    if blocked {
        GeoblockStatus::Blocked
    } else {
        GeoblockStatus::Allowed
    }
}
