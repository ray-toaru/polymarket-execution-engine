use crate::{
    OfficialSdkAdapterConfig, OfficialSdkAdapterError, RealFundsCanaryReceipt,
    RealFundsCanaryRequest, validate_real_funds_canary_preconditions,
};

use super::shared::{authenticated_sdk_client, sdk_call_timeout, signer_from_env};

use anyhow::Context;
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};
use std::str::FromStr;
use tokio::time;

pub async fn run_real_funds_canary_fok_fill(
    config: &OfficialSdkAdapterConfig,
    request: RealFundsCanaryRequest,
) -> anyhow::Result<RealFundsCanaryReceipt> {
    validate_real_funds_canary_preconditions(config, &request)?;

    let signer = signer_from_env()?;
    let client = authenticated_sdk_client(config).await?;
    let timeout = sdk_call_timeout();

    let token_id = SdkU256::from_str(&request.market.token_id)
        .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid token_id: {e}")))?;
    let price = SdkDecimal::from_str(&request.market.limit_price).map_err(|e| {
        OfficialSdkAdapterError::InvalidInput(format!("invalid canary limit_price: {e}"))
    })?;
    let size = SdkDecimal::from_str(&request.market.size)
        .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid canary size: {e}")))?;

    let signable = time::timeout(
        timeout,
        client
            .limit_order()
            .token_id(token_id)
            .price(price)
            .size(size)
            .side(SdkSide::Buy)
            .order_type(SdkOrderType::FOK)
            .post_only(false)
            .build(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK canary order build timed out after {timeout:?}"))?
    .context("official SDK canary order build failed")?;

    let signed = time::timeout(timeout, client.sign(&signer, signable))
        .await
        .map_err(|_| anyhow::anyhow!("official SDK canary order sign timed out after {timeout:?}"))?
        .context("official SDK canary order sign failed")?;

    let response = time::timeout(timeout, client.post_order(signed))
        .await
        .map_err(|_| anyhow::anyhow!("official SDK post_order timed out after {timeout:?}"))?
        .context("official SDK canary post_order failed")?;

    let remote_status = format!("{:?}", response.status);
    let filled_or_matched = remote_status == "Matched";
    Ok(RealFundsCanaryReceipt {
        account_id: request.account_id,
        execution_id: request.execution_id,
        plan_hash: request.plan_hash,
        approval_hash: request.approval.approval_hash,
        idempotency_key: request.idempotency_key,
        remote_order_id: (!response.order_id.is_empty()).then_some(response.order_id),
        remote_status,
        posted: response.success,
        filled_or_matched,
        cancelled: false,
        remote_side_effects: true,
        raw_signed_order_exposed: false,
    })
}
