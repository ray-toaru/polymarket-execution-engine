use crate::{OfficialSdkAdapterError, OfficialSdkOrderBuilderMapping, OfficialSdkPlanOrder};
use pmx_core::{validate_limit_price_decimal_string, validate_positive_decimal_string};

pub fn official_sdk_plan_to_builder_mapping(
    plan: &OfficialSdkPlanOrder,
) -> Result<OfficialSdkOrderBuilderMapping, OfficialSdkAdapterError> {
    let normalized_side = normalize_side(&plan.side)?;
    let normalized_kind = normalize_order_kind(&plan.order_kind)?;
    let normalized_tif = normalize_time_in_force(plan.time_in_force.as_deref(), &normalized_kind)?;
    validate_token_id(&plan.token_id)?;

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
        post_only: plan.post_only.unwrap_or(false),
        builder_attribution: clone_non_empty(plan.builder_attribution.as_deref()),
        fee_rate_bps: clone_non_empty(plan.fee_rate_bps.as_deref()),
        funder: clone_non_empty(plan.funder.as_deref()),
        signer: clone_non_empty(plan.signer.as_deref()),
        signature_type: clone_non_empty(plan.signature_type.as_deref()),
    })
}

fn require_non_empty<'a>(
    value: Option<&'a str>,
    field: &str,
) -> Result<&'a str, OfficialSdkAdapterError> {
    let raw = value
        .ok_or_else(|| OfficialSdkAdapterError::InvalidInput(format!("{field} is required")))?;
    if raw.trim().is_empty() || raw != raw.trim() {
        return Err(OfficialSdkAdapterError::InvalidInput(format!(
            "{field} is required"
        )));
    }
    Ok(raw)
}

fn validate_token_id(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed != raw || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(OfficialSdkAdapterError::InvalidInput(format!(
            "invalid token_id for official SDK order builder: {raw}"
        )));
    }
    Ok(())
}

fn validate_limit_price_for_sdk(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    validate_limit_price_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid limit_price for official SDK order builder: {raw}"
        ))
    })
}

fn validate_positive_quantity_for_sdk(
    raw: &str,
    field: &str,
) -> Result<(), OfficialSdkAdapterError> {
    validate_positive_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid {field} for official SDK order builder: {raw}"
        ))
    })
}

fn clone_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_order_kind(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "LIMIT" | "MARKET" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported order_kind: {raw}"
        ))),
    }
}

fn normalize_side(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "BUY" | "SELL" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

fn normalize_time_in_force(
    raw: Option<&str>,
    order_kind: &str,
) -> Result<Option<String>, OfficialSdkAdapterError> {
    if order_kind == "MARKET" {
        return Ok(None);
    }
    let normalized = raw.unwrap_or("GTC").trim().to_ascii_uppercase();
    match normalized.as_str() {
        "GTC" | "FOK" | "FAK" => Ok(Some(normalized)),
        "IOC" => Ok(Some("FAK".into())),
        "GTD" => Err(OfficialSdkAdapterError::InvalidInput(
            "GTD mapping requires an explicit expiration path that is not wired in v0.20".into(),
        )),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {normalized}"
        ))),
    }
}
