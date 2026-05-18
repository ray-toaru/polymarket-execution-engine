mod normalization;
mod validation;

use crate::{OfficialSdkAdapterError, OfficialSdkOrderBuilderMapping, OfficialSdkPlanOrder};
use normalization::{
    clone_non_empty, normalize_expiration, normalize_order_kind, normalize_side,
    normalize_time_in_force,
};
use validation::{
    require_non_empty, validate_limit_price_for_sdk, validate_positive_quantity_for_sdk,
    validate_token_id,
};

pub fn official_sdk_plan_to_builder_mapping(
    plan: &OfficialSdkPlanOrder,
) -> Result<OfficialSdkOrderBuilderMapping, OfficialSdkAdapterError> {
    let normalized_side = normalize_side(&plan.side)?;
    let normalized_kind = normalize_order_kind(&plan.order_kind)?;
    let normalized_tif = normalize_time_in_force(plan.time_in_force.as_deref(), &normalized_kind)?;
    validate_token_id(&plan.token_id)?;
    let expiration = normalize_expiration(plan.expiration.as_deref(), normalized_tif.as_deref())?;

    match normalized_kind.as_str() {
        "LIMIT" => {
            let limit_price = require_non_empty(plan.limit_price.as_deref(), "limit_price")?;
            let size = require_non_empty(plan.size.as_deref(), "size")?;
            validate_limit_price_for_sdk(limit_price)?;
            validate_positive_quantity_for_sdk(size, "size")?;
        }
        "MARKET" => {
            let amount = require_non_empty(plan.amount.as_deref(), "amount")?;
            validate_positive_quantity_for_sdk(amount, "amount")?;
        }
        _ => unreachable!("normalize_order_kind restricts allowed values"),
    }

    Ok(OfficialSdkOrderBuilderMapping {
        execution_id: plan.execution_id.clone(),
        account_id: plan.account_id.clone(),
        token_id: plan.token_id.clone(),
        side: normalized_side,
        order_kind: normalized_kind,
        limit_price: clone_non_empty(plan.limit_price.as_deref()),
        size: clone_non_empty(plan.size.as_deref()),
        amount: clone_non_empty(plan.amount.as_deref()),
        time_in_force: normalized_tif,
        expiration,
        post_only: plan.post_only.unwrap_or(false),
        builder_attribution: clone_non_empty(plan.builder_attribution.as_deref()),
        fee_rate_bps: clone_non_empty(plan.fee_rate_bps.as_deref()),
        funder: clone_non_empty(plan.funder.as_deref()),
        signer: clone_non_empty(plan.signer.as_deref()),
        signature_type: clone_non_empty(plan.signature_type.as_deref()),
    })
}
