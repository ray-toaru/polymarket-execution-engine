#[cfg(feature = "authenticated-smoke")]
mod authenticated;
#[cfg(feature = "live-submit")]
mod gateway;
#[cfg(feature = "live-submit")]
mod live_canary;
#[cfg(any(
    feature = "authenticated-smoke",
    feature = "sign-only-dry-run",
    feature = "live-submit"
))]
mod shared;
#[cfg(feature = "sign-only-dry-run")]
mod sign_only;
#[cfg(any(
    feature = "authenticated-smoke",
    feature = "sign-only-dry-run",
    feature = "live-submit",
    all(feature = "sdk-typecheck", test)
))]
mod signature_type;

#[cfg(feature = "authenticated-smoke")]
pub use authenticated::run_authenticated_non_trading_sdk_smoke;
#[cfg(feature = "live-submit")]
pub use gateway::{OfficialSdkGateway, OfficialSdkSignerProvider, official_sdk_gateway_pair};
#[cfg(feature = "live-submit")]
pub use live_canary::{
    preflight_real_funds_canary_execution, run_real_funds_canary_gtc_post_only_cancel,
    run_real_funds_canary_gtc_post_only_cancel_with_reporter, validate_real_funds_canary_market,
    validate_real_funds_canary_market_with_diagnostics,
};
#[cfg(feature = "live-submit")]
pub use shared::validate_active_profile_env_for_canary;
#[cfg(feature = "sign-only-dry-run")]
pub use sign_only::run_sign_only_dry_run;

#[cfg(all(feature = "sign-only-dry-run", test))]
pub(crate) use sign_only::discover_active_token_id;
#[cfg(all(feature = "sdk-typecheck", test))]
pub(crate) use signature_type::parse_signature_type_for_test;
