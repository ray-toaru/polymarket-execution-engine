use crate::OfficialSdkAdapterError;

use chrono::{DateTime, Utc};
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};

pub(super) fn parse_sdk_side(raw: &str) -> Result<SdkSide, OfficialSdkAdapterError> {
    match raw {
        "BUY" => Ok(SdkSide::Buy),
        "SELL" => Ok(SdkSide::Sell),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

pub(super) fn parse_sdk_order_type(raw: &str) -> Result<SdkOrderType, OfficialSdkAdapterError> {
    match raw {
        "GTC" => Ok(SdkOrderType::GTC),
        "FOK" => Ok(SdkOrderType::FOK),
        "FAK" => Ok(SdkOrderType::FAK),
        "GTD" => Ok(SdkOrderType::GTD),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {raw}"
        ))),
    }
}

pub(super) fn parse_gtd_expiration(raw: &str) -> Result<DateTime<Utc>, OfficialSdkAdapterError> {
    let parsed = DateTime::parse_from_rfc3339(raw).map_err(|_| {
        OfficialSdkAdapterError::InvalidInput("GTD expiration must be RFC3339".into())
    })?;
    Ok(parsed.with_timezone(&Utc))
}

pub(super) fn signature_fingerprint(signature: &str) -> String {
    let trimmed = signature.strip_prefix("0x").unwrap_or(signature);
    let head = trimmed.get(..16).unwrap_or(trimmed);
    format!("sig-{head}")
}
