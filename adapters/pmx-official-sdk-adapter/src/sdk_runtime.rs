#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use crate::model::{L2_API_KEY_VAR, L2_API_PASSPHRASE_VAR, L2_API_SECRET_VAR};
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use crate::{
    AdapterCredentialSnapshot, ENV_SDK_CALL_TIMEOUT_SECS, OfficialSdkAdapterConfig,
    OfficialSdkAdapterError,
};
#[cfg(feature = "authenticated-smoke")]
use crate::{AuthenticatedNonTradingSmokeReport, validate_authenticated_non_trading_smoke};
#[cfg(feature = "sign-only-dry-run")]
use crate::{
    SignOnlyDryRunReceipt, SignOnlyDryRunRequest, official_sdk_plan_to_builder_mapping,
    validate_sign_only_dry_run,
};

#[cfg(feature = "authenticated-smoke")]
use polymarket_client_sdk_v2::clob::types::AssetType as SdkAssetType;
#[cfg(feature = "sign-only-dry-run")]
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};
#[cfg(feature = "sign-only-dry-run")]
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use anyhow::Context;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::auth::{
    Credentials as SdkCredentials, LocalSigner, Signer as _, Uuid,
};
#[cfg(feature = "authenticated-smoke")]
use polymarket_client_sdk_v2::clob::types::request::BalanceAllowanceRequest;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::clob::{Client as SdkClient, Config as SdkConfig};
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use std::str::FromStr;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use std::time::Duration;
#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
use tokio::time;

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
fn sdk_call_timeout() -> Duration {
    let parsed = std::env::var(ENV_SDK_CALL_TIMEOUT_SECS)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|secs| *secs > 0);
    Duration::from_secs(parsed.unwrap_or(10))
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
fn signer_from_env() -> anyhow::Result<impl polymarket_client_sdk_v2::auth::Signer + Clone> {
    let private_key = std::env::var(PRIVATE_KEY_VAR)
        .with_context(|| format!("missing {PRIVATE_KEY_VAR} for official SDK signer"))?;
    let signer = LocalSigner::from_str(&private_key)
        .context("invalid POLYMARKET_PRIVATE_KEY for official SDK signer")?
        .with_chain_id(Some(POLYGON));
    Ok(signer)
}

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
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

#[cfg(any(feature = "authenticated-smoke", feature = "sign-only-dry-run"))]
async fn authenticated_sdk_client(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<
    SdkClient<
        polymarket_client_sdk_v2::auth::state::Authenticated<
            polymarket_client_sdk_v2::auth::Normal,
        >,
    >,
> {
    let signer = signer_from_env()?;
    let mut builder = SdkClient::new(
        &config.clob_host,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating official SDK client")?
    .authentication_builder(&signer);
    if let Some(credentials) = sdk_credentials_from_env()? {
        builder = builder.credentials(credentials);
    }
    let timeout = sdk_call_timeout();
    let client = time::timeout(timeout, builder.authenticate())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK authentication timed out after {timeout:?}"))?
        .context("official SDK authentication failed")?;
    Ok(client)
}

#[cfg(all(feature = "sign-only-dry-run", test))]
pub(crate) async fn discover_active_token_id(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<String> {
    let client = SdkClient::new(
        &config.clob_host,
        SdkConfig::builder().use_server_time(true).build(),
    )
    .context("creating public official SDK client")?;
    let timeout = sdk_call_timeout();
    let markets = time::timeout(timeout, client.simplified_markets(None))
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK simplified_markets() timed out after {timeout:?}")
        })?
        .context("official SDK simplified_markets() failed")?;

    let token_id = markets
        .data
        .iter()
        .find(|market| {
            market.active && !market.closed && !market.archived && market.accepting_orders
        })
        .and_then(|market| market.tokens.first())
        .map(|token| token.token_id.to_string())
        .or_else(|| {
            markets.data.iter().find_map(|market| {
                market
                    .tokens
                    .first()
                    .map(|token| token.token_id.to_string())
            })
        })
        .ok_or_else(|| {
            anyhow::anyhow!("no simplified market token_id available for sign-only dry-run")
        })?;

    Ok(token_id)
}

#[cfg(feature = "authenticated-smoke")]
pub async fn run_authenticated_non_trading_sdk_smoke(
    config: &OfficialSdkAdapterConfig,
) -> anyhow::Result<AuthenticatedNonTradingSmokeReport> {
    let credentials = AdapterCredentialSnapshot::from_env();
    validate_authenticated_non_trading_smoke(config, &credentials)?;
    if !credentials.has_l1_private_key {
        return Err(OfficialSdkAdapterError::MissingCredential(
            "authenticated non-trading smoke currently requires POLYMARKET_PRIVATE_KEY for SDK authentication".into(),
        )
        .into());
    }

    let client = authenticated_sdk_client(config).await?;
    let timeout = sdk_call_timeout();

    let ok_status = time::timeout(timeout, client.ok())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK ok() timed out after {timeout:?}"))?
        .context("official SDK ok() failed")?;
    let server_time = time::timeout(timeout, client.server_time())
        .await
        .map_err(|_| anyhow::anyhow!("official SDK server_time() timed out after {timeout:?}"))?
        .context("official SDK server_time() failed")?;
    let readonly_api_keys = time::timeout(timeout, client.readonly_api_keys())
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK readonly_api_keys() timed out after {timeout:?}")
        })?
        .context("official SDK readonly_api_keys() failed")?;
    let closed_only = time::timeout(timeout, client.closed_only_mode())
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK closed_only_mode() timed out after {timeout:?}")
        })?
        .context("official SDK closed_only_mode() failed")?;
    let _balance_allowance = time::timeout(
        timeout,
        client.balance_allowance(
            BalanceAllowanceRequest::builder()
                .asset_type(SdkAssetType::Collateral)
                .build(),
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK balance_allowance() timed out after {timeout:?}"))?
    .context("official SDK balance_allowance() failed")?;

    Ok(AuthenticatedNonTradingSmokeReport {
        ok_status,
        server_time,
        api_key_count: readonly_api_keys.len(),
        closed_only: closed_only.closed_only,
        balance_allowance_checked: true,
        credential_snapshot: credentials,
    })
}

#[cfg(feature = "sign-only-dry-run")]
pub async fn run_sign_only_dry_run(
    config: &OfficialSdkAdapterConfig,
    request: SignOnlyDryRunRequest,
) -> anyhow::Result<SignOnlyDryRunReceipt> {
    let credentials = AdapterCredentialSnapshot::from_env();
    validate_sign_only_dry_run(config, &credentials)?;

    let mapping = official_sdk_plan_to_builder_mapping(&request.clone().into_plan_order())?;
    let signer = signer_from_env()?;
    let client = authenticated_sdk_client(config).await?;
    let timeout = sdk_call_timeout();

    let token_id = SdkU256::from_str(&mapping.token_id)
        .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid token_id: {e}")))?;
    let price = SdkDecimal::from_str(
        mapping
            .limit_price
            .as_deref()
            .ok_or_else(|| OfficialSdkAdapterError::InvalidInput("missing limit_price".into()))?,
    )
    .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid limit_price: {e}")))?;
    let size = SdkDecimal::from_str(
        mapping
            .size
            .as_deref()
            .ok_or_else(|| OfficialSdkAdapterError::InvalidInput("missing size".into()))?,
    )
    .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid size: {e}")))?;
    let side = parse_sdk_side(&mapping.side)?;
    let order_type =
        parse_sdk_order_type(mapping.time_in_force.as_deref().ok_or_else(|| {
            OfficialSdkAdapterError::InvalidInput("missing time_in_force".into())
        })?)?;

    let signable = time::timeout(
        timeout,
        client
            .limit_order()
            .token_id(token_id)
            .price(price)
            .size(size)
            .side(side)
            .order_type(order_type)
            .post_only(mapping.post_only)
            .build(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK limit_order().build() timed out after {timeout:?}"))?
    .context("official SDK limit order build failed")?;

    let signed = time::timeout(timeout, client.sign(&signer, signable))
        .await
        .map_err(|_| anyhow::anyhow!("official SDK sign() timed out after {timeout:?}"))?
        .context("official SDK sign() failed")?;

    let signed_order_ref = format!(
        "sign-only:{}:{}:{}",
        request.execution_id.0,
        request.plan_hash.0,
        signature_fingerprint(&signed.signature.to_string())
    );

    Ok(SignOnlyDryRunReceipt {
        account_id: request.account_id,
        execution_id: request.execution_id,
        plan_hash: request.plan_hash,
        signed_order_ref,
        posted: false,
    })
}

#[cfg(feature = "sign-only-dry-run")]
fn parse_sdk_side(raw: &str) -> Result<SdkSide, OfficialSdkAdapterError> {
    match raw {
        "BUY" => Ok(SdkSide::Buy),
        "SELL" => Ok(SdkSide::Sell),
        _ => Err(OfficialSdkAdapterError::InvalidInput(format!(
            "unsupported side: {raw}"
        ))),
    }
}

#[cfg(feature = "sign-only-dry-run")]
fn parse_sdk_order_type(raw: &str) -> Result<SdkOrderType, OfficialSdkAdapterError> {
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

#[cfg(feature = "sign-only-dry-run")]
fn signature_fingerprint(signature: &str) -> String {
    let trimmed = signature.strip_prefix("0x").unwrap_or(signature);
    let head = trimmed.get(..16).unwrap_or(trimmed);
    format!("sig-{head}")
}
