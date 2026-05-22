use anyhow::Context;
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
use polymarket_client_sdk_v2::clob::types::SignatureType;
use polymarket_client_sdk_v2::clob::types::request::TradesRequest;
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
use polymarket_client_sdk_v2::types::{Address, U256};
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
struct TradeSummary {
    id: String,
    taker_order_id: String,
    side: String,
    size: String,
    price: String,
    status: String,
    trader_side: String,
    outcome: String,
    asset_id: String,
    market: String,
    match_time: String,
    last_update: String,
}

#[derive(Debug, Serialize)]
struct TradeQueryReport {
    query_performed: bool,
    token_id: String,
    order_id_filter: Option<String>,
    total_trades_returned: usize,
    matching_trades_count: usize,
    matching_size_total: String,
    no_matching_fills_observed: bool,
    trades: Vec<TradeSummary>,
    error_summary: Option<String>,
    raw_signed_order_exposed: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
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

    let token_id = U256::from_str(&args.token_id).context("invalid --token-id")?;
    let request = TradesRequest::builder().asset_id(token_id).build();
    let report = match client.trades(&request, None).await {
        Ok(page) => {
            let trades: Vec<_> = page
                .data
                .into_iter()
                .map(|trade| TradeSummary {
                    id: trade.id,
                    taker_order_id: trade.taker_order_id,
                    side: trade.side.to_string(),
                    size: trade.size.to_string(),
                    price: trade.price.to_string(),
                    status: format!("{:?}", trade.status),
                    trader_side: format!("{:?}", trade.trader_side),
                    outcome: trade.outcome,
                    asset_id: trade.asset_id.to_string(),
                    market: trade.market.to_string(),
                    match_time: trade.match_time.to_rfc3339(),
                    last_update: trade.last_update.to_rfc3339(),
                })
                .collect();
            let matching: Vec<_> = trades
                .iter()
                .filter(|trade| {
                    args.order_id
                        .as_ref()
                        .is_none_or(|order_id| trade.taker_order_id == *order_id)
                })
                .collect();
            TradeQueryReport {
                query_performed: true,
                token_id: args.token_id,
                order_id_filter: args.order_id,
                total_trades_returned: trades.len(),
                matching_trades_count: matching.len(),
                matching_size_total: if matching.is_empty() {
                    "0".into()
                } else {
                    matching
                        .iter()
                        .map(|trade| trade.size.as_str())
                        .collect::<Vec<_>>()
                        .join("+")
                },
                no_matching_fills_observed: matching.is_empty(),
                trades,
                error_summary: None,
                raw_signed_order_exposed: false,
            }
        }
        Err(err) => TradeQueryReport {
            query_performed: true,
            token_id: args.token_id,
            order_id_filter: args.order_id,
            total_trades_returned: 0,
            matching_trades_count: 0,
            matching_size_total: String::new(),
            no_matching_fills_observed: false,
            trades: Vec::new(),
            error_summary: Some(redact_error(&err.to_string())),
            raw_signed_order_exposed: false,
        },
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Debug)]
struct Args {
    token_id: String,
    order_id: Option<String>,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut token_id = None;
    let mut order_id = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--token-id" => {
                token_id = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --token-id"))?,
                );
            }
            "--order-id" => {
                order_id = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --order-id"))?,
                );
            }
            _ => anyhow::bail!("unknown argument {arg}"),
        }
    }
    Ok(Args {
        token_id: token_id.ok_or_else(|| anyhow::anyhow!("missing required --token-id"))?,
        order_id,
    })
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
