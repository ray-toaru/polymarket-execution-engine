use serde::{Deserialize, Serialize};

use super::{CLOB_PRODUCTION_HOST, CLOB_V2_COLLATERAL_SYMBOL, CLOB_V2_SIGNING_PROTOCOL};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkAdapterConfig {
    pub clob_host: String,
    pub allow_read_only_smoke: bool,
    pub allow_authenticated_non_trading_smoke: bool,
    pub allow_sign_only_dry_run: bool,
    pub allow_live_submit: bool,
    pub allow_real_funds_canary: bool,
    pub require_kill_switch_open_for_live_submit: bool,
    pub require_repository_reservation_for_live_submit: bool,
    pub require_reconcile_worker_for_live_submit: bool,
}

impl Default for OfficialSdkAdapterConfig {
    fn default() -> Self {
        Self {
            clob_host: CLOB_PRODUCTION_HOST.to_string(),
            allow_read_only_smoke: true,
            allow_authenticated_non_trading_smoke: false,
            allow_sign_only_dry_run: false,
            allow_live_submit: false,
            allow_real_funds_canary: false,
            require_kill_switch_open_for_live_submit: true,
            require_repository_reservation_for_live_submit: true,
            require_reconcile_worker_for_live_submit: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyProfile {
    pub clob_host: String,
    pub collateral_symbol: String,
    pub signing_protocol: String,
    pub uses_deposit_wallet_order_path: bool,
    pub supports_builder_attribution: bool,
    pub supports_fee_metadata: bool,
    pub exposes_raw_signed_order: bool,
    pub may_post_order: bool,
    pub may_cancel_order: bool,
}

impl Default for OfficialSdkStandardSignOnlyProfile {
    fn default() -> Self {
        Self {
            clob_host: CLOB_PRODUCTION_HOST.into(),
            collateral_symbol: CLOB_V2_COLLATERAL_SYMBOL.into(),
            signing_protocol: CLOB_V2_SIGNING_PROTOCOL.into(),
            uses_deposit_wallet_order_path: true,
            supports_builder_attribution: true,
            supports_fee_metadata: true,
            exposes_raw_signed_order: false,
            may_post_order: false,
            may_cancel_order: false,
        }
    }
}
