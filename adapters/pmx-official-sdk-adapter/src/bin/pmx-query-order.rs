use anyhow::Context;
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
use polymarket_client_sdk_v2::clob::types::SignatureType;
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
use polymarket_client_sdk_v2::types::Address;
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
use serde::Serialize;
use std::str::FromStr;

const CLOB_PRODUCTION_HOST: &str = "https://clob.polymarket.com/";
const L2_API_KEY_VAR: &str = "POLY_API_KEY";
const L2_API_SECRET_VAR: &str = "POLY_API_SECRET";
const L2_API_PASSPHRASE_VAR: &str = "POLY_API_PASSPHRASE";
const ENV_CLOB_FUNDER: &str = "PMX_CLOB_FUNDER";
const ENV_CLOB_SIGNATURE_TYPE: &str = "PMX_CLOB_SIGNATURE_TYPE";

#[derive(Debug, Serialize)]
struct OrderQueryReport {
    order_id: String,
    query_performed: bool,
    open_order_found: bool,
    normalized_status: String,
    remote_status: Option<String>,
    side: Option<String>,
    order_type: Option<String>,
    price: Option<String>,
    original_size: Option<String>,
    size_matched: Option<String>,
    error_summary: Option<String>,
    raw_signed_order_exposed: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let order_id = match args.next().as_deref() {
        Some("--order-id") => args
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing value for --order-id"))?,
        _ => anyhow::bail!("usage: pmx-query-order --order-id <order-id>"),
    };
    if args.next().is_some() {
        anyhow::bail!("unexpected trailing argument");
    }

    let signer = signer_from_env()?;
    let mut builder = SdkClient::new(
        CLOB_PRODUCTION_HOST,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating official SDK client")?
    .authentication_builder(&signer);
    if let Some(funder) = clob_funder_from_env()? {
        builder = builder
            .funder(funder)
            .signature_type(clob_signature_type_from_env()?);
    } else {
        builder = builder.signature_type(clob_signature_type_from_env()?);
    }
    if let Some(credentials) = sdk_credentials_from_env()? {
        builder = builder.credentials(credentials);
    }
    let client = builder
        .authenticate()
        .await
        .context("SDK authentication failed")?;

    let report = match client.order(&order_id).await {
        Ok(order) => OrderQueryReport {
            order_id,
            query_performed: true,
            open_order_found: true,
            normalized_status: "open_order_found".into(),
            remote_status: Some(order.status.to_string()),
            side: Some(order.side.to_string()),
            order_type: Some(order.order_type.to_string()),
            price: Some(order.price.to_string()),
            original_size: Some(order.original_size.to_string()),
            size_matched: Some(order.size_matched.to_string()),
            error_summary: None,
            raw_signed_order_exposed: false,
        },
        Err(err) => OrderQueryReport {
            order_id,
            query_performed: true,
            open_order_found: false,
            normalized_status: "not_open_or_not_returned_by_open_order_endpoint".into(),
            remote_status: None,
            side: None,
            order_type: None,
            price: None,
            original_size: None,
            size_matched: None,
            error_summary: Some(redact_error(&err.to_string())),
            raw_signed_order_exposed: false,
        },
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn signer_from_env() -> anyhow::Result<impl polymarket_client_sdk_v2::auth::Signer + Clone> {
    let private_key = std::env::var(PRIVATE_KEY_VAR)
        .with_context(|| format!("missing {PRIVATE_KEY_VAR} for official SDK signer"))?;
    Ok(LocalSigner::from_str(&private_key)
        .context("invalid POLYMARKET_PRIVATE_KEY for official SDK signer")?
        .with_chain_id(Some(POLYGON)))
}

fn sdk_credentials_from_env() -> anyhow::Result<Option<SdkCredentials>> {
    match (
        std::env::var(L2_API_KEY_VAR).ok(),
        std::env::var(L2_API_SECRET_VAR).ok(),
        std::env::var(L2_API_PASSPHRASE_VAR).ok(),
    ) {
        (Some(key), Some(secret), Some(passphrase)) => {
            let uuid = Uuid::parse_str(&key).context("invalid POLY_API_KEY UUID")?;
            Ok(Some(SdkCredentials::new(uuid, secret, passphrase)))
        }
        (None, None, None) => Ok(None),
        _ => Err(anyhow::anyhow!(
            "partial L2 credential material present; require POLY_API_KEY, POLY_API_SECRET and POLY_API_PASSPHRASE together"
        )),
    }
}

fn clob_funder_from_env() -> anyhow::Result<Option<Address>> {
    std::env::var(ENV_CLOB_FUNDER)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Address::from_str(value.trim())
                .with_context(|| format!("invalid {ENV_CLOB_FUNDER} address"))
        })
        .transpose()
}

fn clob_signature_type_from_env() -> anyhow::Result<SignatureType> {
    let Some(raw) = std::env::var(ENV_CLOB_SIGNATURE_TYPE)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(SignatureType::Eoa);
    };
    match raw.trim().to_ascii_uppercase().as_str() {
        "EOA" | "0" => Ok(SignatureType::Eoa),
        "PROXY" | "POLY_PROXY" | "1" => Ok(SignatureType::Proxy),
        "GNOSIS_SAFE" | "GNOSISSAFE" | "POLY_GNOSIS_SAFE" | "2" => Ok(SignatureType::GnosisSafe),
        "POLY_1271" | "POLY1271" | "DEPOSIT_WALLET" | "3" => Ok(SignatureType::Poly1271),
        _ => Err(anyhow::anyhow!(
            "unsupported {ENV_CLOB_SIGNATURE_TYPE} value"
        )),
    }
}

fn redact_error(value: &str) -> String {
    let mut redacted = value.to_string();
    for key in [
        PRIVATE_KEY_VAR,
        L2_API_KEY_VAR,
        L2_API_SECRET_VAR,
        L2_API_PASSPHRASE_VAR,
        ENV_CLOB_FUNDER,
    ] {
        redacted = redacted.replace(key, "[REDACTED_KEY]");
    }
    redacted.chars().take(240).collect()
}
