use crate::OfficialSdkAdapterError;

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
        "GTD" => Err(OfficialSdkAdapterError::InvalidInput(
            "GTD sign-only is not wired in v0.20".into(),
        )),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported time_in_force: {raw}"
        ))),
    }
}

pub(super) fn signature_fingerprint(signature: &str) -> String {
    let trimmed = signature.strip_prefix("0x").unwrap_or(signature);
    let head = trimmed.get(..16).unwrap_or(trimmed);
    format!("sig-{head}")
}
