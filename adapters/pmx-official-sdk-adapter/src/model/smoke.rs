use serde::{Deserialize, Serialize};

use super::AdapterCredentialSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedNonTradingSmokeReport {
    pub ok_status: String,
    pub server_time: i64,
    pub api_key_count: usize,
    pub closed_only: bool,
    pub balance_allowance_checked: bool,
    pub credential_snapshot: AdapterCredentialSnapshot,
}
