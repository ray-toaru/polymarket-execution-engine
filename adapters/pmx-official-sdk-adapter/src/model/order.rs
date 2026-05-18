use pmx_core::{AccountId, ExecutionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkPlanOrder {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub token_id: String,
    pub side: String,
    pub order_kind: String,
    pub limit_price: Option<String>,
    pub size: Option<String>,
    pub amount: Option<String>,
    pub time_in_force: Option<String>,
    pub post_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder_attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_rate_bps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkOrderBuilderMapping {
    pub execution_id: ExecutionId,
    pub account_id: AccountId,
    pub token_id: String,
    pub side: String,
    pub order_kind: String,
    pub limit_price: Option<String>,
    pub size: Option<String>,
    pub amount: Option<String>,
    pub time_in_force: Option<String>,
    pub post_only: bool,
    pub builder_attribution: Option<String>,
    pub fee_rate_bps: Option<String>,
    pub funder: Option<String>,
    pub signer: Option<String>,
    pub signature_type: Option<String>,
}
