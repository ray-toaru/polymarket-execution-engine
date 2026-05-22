pub const OFFICIAL_SDK_REPOSITORY: &str = "https://github.com/Polymarket/rs-clob-client-v2";
pub const OFFICIAL_SDK_CRATE: &str = "polymarket_client_sdk_v2";
pub const PINNED_OFFICIAL_SDK_VERSION: &str = "=0.6.0-canary.1";
pub const CLOB_PRODUCTION_HOST: &str = "https://clob.polymarket.com";
pub const LEGACY_CLOB_V2_REDIRECT_HOST: &str = "https://clob-v2.polymarket.com";
pub const ENV_RUN_AUTHENTICATED_SMOKE: &str = "PMX_RUN_AUTHENTICATED_NON_TRADING_SMOKE";
pub const ENV_RUN_SIGN_ONLY_DRY_RUN: &str = "PMX_RUN_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_SIGN_ONLY_DRY_RUN: &str = "PMX_ALLOW_SIGN_ONLY_DRY_RUN";
pub const ENV_ALLOW_LIVE_SUBMIT: &str = "PMX_ALLOW_LIVE_SUBMIT";
pub const ENV_ALLOW_LIVE_CANCEL: &str = "PMX_ALLOW_LIVE_CANCEL";
pub const ENV_ALLOW_REAL_FUNDS_CANARY: &str = "PMX_ALLOW_REAL_FUNDS_CANARY";
pub const ENV_SDK_CALL_TIMEOUT_SECS: &str = "PMX_SDK_CALL_TIMEOUT_SECS";
pub const REDACTED: &str = "[REDACTED]";
pub const CLOB_V2_COLLATERAL_SYMBOL: &str = "pUSD";
pub const CLOB_V2_SIGNING_PROTOCOL: &str = "CLOB_V2";

pub(crate) const PRIVATE_KEY_VAR_NAME: &str = "POLYMARKET_PRIVATE_KEY";
pub(crate) const L2_API_KEY_VAR: &str = "POLY_API_KEY";
pub(crate) const L2_API_SECRET_VAR: &str = "POLY_API_SECRET";
pub(crate) const L2_API_PASSPHRASE_VAR: &str = "POLY_API_PASSPHRASE";

pub(crate) fn env_present(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| !value.trim().is_empty())
}

pub(crate) fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

pub fn is_canonical_production_clob_host(host: &str) -> bool {
    host.trim().trim_end_matches('/') == CLOB_PRODUCTION_HOST
}
