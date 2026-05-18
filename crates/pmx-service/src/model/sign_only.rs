use pmx_core::SignOnlyLifecycleRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionRequest {
    pub execution_id: String,
    pub account_id: String,
    pub plan_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_order_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_order_digest: Option<String>,
    pub no_remote_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionReceipt {
    pub execution_id: String,
    pub signed_order_ref: String,
    pub signed_order_digest: Option<String>,
    pub lifecycle_records: Vec<SignOnlyLifecycleRecord>,
    pub no_remote_side_effect: bool,
}
