use pmx_core::ExecutionPlanSummary;

use crate::{ServiceError, StandardSignOnlyConstructionRequest};

pub(super) fn validate_standard_sign_only_request(
    req: &StandardSignOnlyConstructionRequest,
) -> Result<(), ServiceError> {
    if req.execution_id.trim().is_empty()
        || req.account_id.trim().is_empty()
        || req.plan_hash.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "execution_id, account_id and plan_hash must be non-empty".into(),
        ));
    }
    if !req.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "standard sign-only construction must not contain remote side effects".into(),
        ));
    }
    if let Some(signed_order_ref) = req.signed_order_ref.as_deref()
        && !signed_order_ref.starts_with("sign-only:")
    {
        return Err(ServiceError::BadRequest(
            "standard sign-only construction requires a redacted sign-only ref".into(),
        ));
    }
    if let Some(digest) = req.signed_order_digest.as_deref()
        && (digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        return Err(ServiceError::BadRequest(
            "signed_order_digest must be a 64-character hex SHA-256 digest".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_standard_sign_only_plan_match(
    plan: &ExecutionPlanSummary,
    req: &StandardSignOnlyConstructionRequest,
) -> Result<(), ServiceError> {
    if plan.account_id.0 != req.account_id {
        return Err(ServiceError::Conflict(
            "sign-only construction account_id does not match execution plan".into(),
        ));
    }
    if plan.plan_hash.0 != req.plan_hash {
        return Err(ServiceError::Conflict(
            "sign-only construction plan_hash does not match execution plan".into(),
        ));
    }
    Ok(())
}
