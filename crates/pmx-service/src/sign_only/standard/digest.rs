use pmx_core::{ExecutionPlanSummary, canonical_json_sha256};

use crate::ServiceError;

pub(super) fn resolve_signed_order_digest(
    plan: &ExecutionPlanSummary,
    provided_digest: Option<String>,
) -> Result<String, ServiceError> {
    match provided_digest {
        Some(digest) => Ok(digest),
        None => derive_standard_sign_only_digest(plan),
    }
}

pub(super) fn resolve_signed_order_ref(
    execution_id: &str,
    plan_hash: &str,
    provided_ref: Option<String>,
    signed_order_digest: &str,
) -> String {
    provided_ref.unwrap_or_else(|| {
        format!(
            "sign-only:{}:{}:digest-{}",
            execution_id,
            plan_hash,
            &signed_order_digest[..16]
        )
    })
}

fn derive_standard_sign_only_digest(plan: &ExecutionPlanSummary) -> Result<String, ServiceError> {
    let payload = serde_json::json!({
        "schema_version": 1,
        "construction_source": "official-sdk-standard-sign-only",
        "execution_id": plan.execution_id,
        "account_id": plan.account_id,
        "plan_hash": plan.plan_hash,
        "profile": {
            "signed_order_ref_namespace": "sign-only",
            "exposes_raw_signed_order": false,
            "may_post_order": false,
            "may_cancel_order": false,
            "no_remote_side_effect": true
        }
    });
    canonical_json_sha256(&payload)
        .map(|hash| hash.0)
        .map_err(|err| ServiceError::Internal(err.to_string()))
}
