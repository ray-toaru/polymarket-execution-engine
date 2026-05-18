use crate::{
    AdapterCredentialSnapshot, OfficialSdkAdapterConfig, OfficialSdkAdapterError,
    SignOnlyDryRunReceipt, SignOnlyDryRunRequest, official_sdk_plan_to_builder_mapping,
    validate_sign_only_dry_run,
};

use super::super::shared::{authenticated_sdk_client, sdk_call_timeout, signer_from_env};
use super::parsing::{parse_sdk_order_type, parse_sdk_side, signature_fingerprint};

use anyhow::Context;
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};
use std::str::FromStr;
use tokio::time;

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
