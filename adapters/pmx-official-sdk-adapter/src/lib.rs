//! Official Polymarket SDK adapter boundary.
//!
//! This crate is the promotion target after the isolated SDK spike. It remains
//! outside the default execution-engine workspace so `pmx-core`, `pmx-policy`,
//! `pmx-store`, and the Python control plane cannot accidentally gain signing
//! or live trading capability.
//!
//! Safety posture:
//! - read-only SDK calls may be smoke-tested with no credentials;
//! - authenticated non-trading calls require explicit opt-in and real credentials;
//! - sign-only dry-runs require explicit opt-in and must never call `post_order`;
//! - live submit requires the explicit `live-submit` feature and runtime safety gates.

mod gates;
mod lifecycle;
mod liveness;
mod mapping;
mod model;
mod redaction;
mod sdk_runtime;
mod standard_sign_only;

pub use gates::*;
pub use lifecycle::*;
pub use liveness::*;
pub use mapping::*;
pub use model::*;
pub use redaction::*;
#[cfg(feature = "authenticated-smoke")]
pub use sdk_runtime::run_authenticated_non_trading_sdk_smoke;
#[cfg(feature = "sign-only-dry-run")]
pub use sdk_runtime::run_sign_only_dry_run;
#[cfg(feature = "live-submit")]
pub use sdk_runtime::{
    OfficialSdkGateway, OfficialSdkSignerProvider, official_sdk_gateway_pair,
    run_real_funds_canary_gtc_post_only_cancel,
    run_real_funds_canary_gtc_post_only_cancel_with_reporter, validate_real_funds_canary_market,
    validate_real_funds_canary_market_with_diagnostics,
};
pub use standard_sign_only::*;

#[cfg(all(feature = "sign-only-dry-run", test))]
pub(crate) use sdk_runtime::discover_active_token_id;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
