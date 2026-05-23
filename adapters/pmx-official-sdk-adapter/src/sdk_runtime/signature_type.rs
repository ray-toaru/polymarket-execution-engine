use polymarket_client_sdk_v2::clob::types::SignatureType;

pub(super) const ENV_CLOB_SIGNATURE_TYPE: &str = "PMX_CLOB_SIGNATURE_TYPE";

pub(super) fn parse_signature_type(raw: &str) -> Result<SignatureType, String> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "EOA" | "0" => Ok(SignatureType::Eoa),
        "PROXY" | "POLY_PROXY" | "1" => Ok(SignatureType::Proxy),
        "GNOSIS_SAFE" | "GNOSISSAFE" | "POLY_GNOSIS_SAFE" | "2" => Ok(SignatureType::GnosisSafe),
        "POLY_1271" | "POLY1271" | "DEPOSIT_WALLET" | "3" => Ok(SignatureType::Poly1271),
        _ => Err(format!("unsupported {ENV_CLOB_SIGNATURE_TYPE} value")),
    }
}

#[cfg(test)]
pub(crate) fn parse_signature_type_for_test(raw: &str) -> Result<SignatureType, String> {
    parse_signature_type(raw)
}
