use crate::{
    AdapterCredentialSnapshot, AuthenticatedNonTradingSmokeReport, OfficialSdkAdapterConfig,
    OfficialSdkAdapterError, validate_authenticated_non_trading_smoke,
};

use super::shared::{authenticated_sdk_client, sdk_call_timeout};

use anyhow::Context;
use polymarket_client_sdk_v2::clob::types::AssetType as SdkAssetType;
use polymarket_client_sdk_v2::clob::types::request::BalanceAllowanceRequest;
use tokio::time;

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
