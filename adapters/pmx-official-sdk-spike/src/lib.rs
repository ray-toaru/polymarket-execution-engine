//! Experimental official SDK integration boundary.
//!
//! This crate remains outside the default workspace live path because it is the
//! official SDK boundary. The main execution engine now aligns its Rust baseline
//! with the SDK (Rust 1.88 / edition 2024), but real trading remains gated by
//! execution-plane policy, store, reservation, and live-submit controls.

use serde::{Deserialize, Serialize};

pub const OFFICIAL_SDK_REPOSITORY: &str = "https://github.com/Polymarket/rs-clob-client-v2";
pub const OFFICIAL_SDK_CRATE: &str = "polymarket_client_sdk_v2";
pub const PINNED_OFFICIAL_SDK_VERSION: &str = "=0.6.0-canary.1";
pub const LIVE_SUBMIT_FEATURE_NAME: &str = "live-submit";
pub const CLOB_V2_HOST: &str = "https://clob-v2.polymarket.com";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialSdkAdapterConfig {
    pub clob_host: String,
    pub use_ws: bool,
    pub use_heartbeats: bool,
    pub allow_live_submit: bool,
    pub require_explicit_runtime_kill_switch_open: bool,
}

impl Default for OfficialSdkAdapterConfig {
    fn default() -> Self {
        Self {
            clob_host: CLOB_V2_HOST.to_string(),
            use_ws: true,
            use_heartbeats: true,
            allow_live_submit: false,
            require_explicit_runtime_kill_switch_open: true,
        }
    }
}

#[cfg(feature = "sdk-typecheck")]
pub fn sdk_client_type_marker() -> &'static str {
    // This deliberately avoids assuming a constructor shape. It only verifies
    // that the official SDK dependency resolves and exposes the documented CLOB
    // client type behind the isolated sdk-typecheck feature.
    std::any::type_name::<polymarket_client_sdk_v2::clob::Client>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "sdk-typecheck")]
    use polymarket_client_sdk_v2::Result as SdkResult;
    #[cfg(feature = "sdk-typecheck")]
    use std::time::Duration;
    #[cfg(feature = "sdk-typecheck")]
    use tokio::time;

    #[cfg(feature = "sdk-typecheck")]
    fn default_read_only_client() -> SdkResult<polymarket_client_sdk_v2::clob::Client> {
        polymarket_client_sdk_v2::clob::Client::new(
            CLOB_V2_HOST,
            polymarket_client_sdk_v2::clob::Config::default(),
        )
    }

    #[test]
    fn live_submit_is_disabled_by_default() {
        let cfg = OfficialSdkAdapterConfig::default();
        assert!(!cfg.allow_live_submit);
        assert_eq!(cfg.clob_host, CLOB_V2_HOST);
        assert!(cfg.require_explicit_runtime_kill_switch_open);
    }

    #[cfg(feature = "sdk-typecheck")]
    #[tokio::test]
    async fn read_only_ok_smoke() -> anyhow::Result<()> {
        // Read-only smoke must not use credential material, but local developer environments may
        // have credentials exported for later authenticated smoke tests. This test deliberately
        // constructs an unauthenticated client and does not inspect or consume secret variables.
        let client = default_read_only_client()?;

        let status = time::timeout(Duration::from_secs(10), client.ok())
            .await
            .map_err(|_| anyhow::anyhow!("SDK read-only smoke timeout"))??;
        assert_eq!(status.to_uppercase(), "OK");

        let _ = time::timeout(Duration::from_secs(10), client.server_time())
            .await
            .map_err(|_| anyhow::anyhow!("SDK read-only smoke server time timeout"))??;

        Ok(())
    }
}
