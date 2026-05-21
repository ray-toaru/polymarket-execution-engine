#[cfg(feature = "authenticated-smoke")]
mod authenticated;
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

#[cfg(feature = "authenticated-smoke")]
pub use authenticated::run_authenticated_non_trading_sdk_smoke;
#[cfg(feature = "live-submit")]
pub use live_canary::{
    run_real_funds_canary_fok_fill, validate_real_funds_canary_market,
    validate_real_funds_canary_market_with_diagnostics,
};
#[cfg(feature = "sign-only-dry-run")]
pub use sign_only::run_sign_only_dry_run;

#[cfg(all(feature = "sign-only-dry-run", test))]
pub(crate) use sign_only::discover_active_token_id;
