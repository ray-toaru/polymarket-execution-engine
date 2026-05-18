use chrono::DateTime;

use crate::OfficialSdkAdapterError;

pub(super) fn clone_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn normalize_order_kind(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "LIMIT" | "MARKET" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported order_kind: {raw}"
        ))),
    }
}

pub(super) fn normalize_side(raw: &str) -> Result<String, OfficialSdkAdapterError> {
    let normalized = raw.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "BUY" | "SELL" => Ok(normalized),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

pub(super) fn normalize_time_in_force(
    raw: Option<&str>,
    order_kind: &str,
) -> Result<Option<String>, OfficialSdkAdapterError> {
    if order_kind == "MARKET" {
        return Ok(None);
    }
    let normalized = raw.unwrap_or("GTC").trim().to_ascii_uppercase();
    match normalized.as_str() {
        "GTC" | "GTD" | "FOK" | "FAK" => Ok(Some(normalized)),
        "IOC" => Ok(Some("FAK".into())),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {normalized}"
        ))),
    }
}

pub(super) fn normalize_expiration(
    raw: Option<&str>,
    time_in_force: Option<&str>,
) -> Result<Option<String>, OfficialSdkAdapterError> {
    let value = clone_non_empty(raw);
    match (time_in_force, value) {
        (Some("GTD"), Some(expiration)) => {
            DateTime::parse_from_rfc3339(&expiration).map_err(|_| {
                OfficialSdkAdapterError::InvalidInput(
                    "GTD expiration must be an RFC3339 timestamp".into(),
                )
            })?;
            Ok(Some(expiration))
        }
        (Some("GTD"), None) => Err(OfficialSdkAdapterError::InvalidInput(
            "GTD mapping requires expiration".into(),
        )),
        (_, Some(_)) => Err(OfficialSdkAdapterError::InvalidInput(
            "expiration is only supported for GTD orders".into(),
        )),
        (_, None) => Ok(None),
    }
}
