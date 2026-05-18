use crate::OfficialSdkAdapterError;
use pmx_core::{validate_limit_price_decimal_string, validate_positive_decimal_string};

pub(super) fn require_non_empty<'a>(
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

pub(super) fn validate_token_id(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed != raw || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(OfficialSdkAdapterError::InvalidInput(format!(
            "invalid token_id for official SDK order builder: {raw}"
        )));
    }
    Ok(())
}

pub(super) fn validate_limit_price_for_sdk(raw: &str) -> Result<(), OfficialSdkAdapterError> {
    validate_limit_price_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid limit_price for official SDK order builder: {raw}"
        ))
    })
}

pub(super) fn validate_positive_quantity_for_sdk(
    raw: &str,
    field: &str,
) -> Result<(), OfficialSdkAdapterError> {
    validate_positive_decimal_string(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput(format!(
            "invalid {field} for official SDK order builder: {raw}"
        ))
    })
}
