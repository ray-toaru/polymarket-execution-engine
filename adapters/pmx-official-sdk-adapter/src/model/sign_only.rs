use pmx_core::{AccountId, ExecutionId, HashValue, SignOnlyLifecycleRecord};
use serde::{Deserialize, Serialize};

use super::{
    OfficialSdkOrderBuilderMapping, OfficialSdkPlanOrder, OfficialSdkStandardSignOnlyProfile,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyDryRunRequest {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub token_id: String,
    pub side: String,
    pub size: String,
    pub limit_price: String,
}

impl SignOnlyDryRunRequest {
    pub fn into_plan_order(self) -> OfficialSdkPlanOrder {
        OfficialSdkPlanOrder {
            execution_id: self.execution_id,
            account_id: self.account_id,
            token_id: self.token_id,
            side: self.side,
            order_kind: "LIMIT".into(),
            limit_price: Some(self.limit_price),
            size: Some(self.size),
            amount: None,
            time_in_force: Some("GTC".into()),
            post_only: Some(false),
            builder_attribution: None,
            fee_rate_bps: None,
            funder: None,
            signer: None,
            signature_type: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignOnlyDryRunReceipt {
    pub account_id: AccountId,
    pub execution_id: ExecutionId,
    pub plan_hash: HashValue,
    pub signed_order_ref: String,
    pub posted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyPlan {
    pub profile: OfficialSdkStandardSignOnlyProfile,
    pub mapping: OfficialSdkOrderBuilderMapping,
    pub signed_order_ref_namespace: String,
    pub exposes_raw_signed_order: bool,
    pub may_post_order: bool,
    pub may_cancel_order: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSdkStandardSignOnlyConstruction {
    pub plan: OfficialSdkStandardSignOnlyPlan,
    pub plan_hash: HashValue,
    pub signed_order_ref: String,
    pub signed_order_digest: String,
    pub no_remote_side_effect: bool,
    pub raw_signed_order_exposed: bool,
    pub lifecycle_records: Vec<SignOnlyLifecycleRecord>,
}
