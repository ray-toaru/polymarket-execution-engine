use crate::{
    OfficialSdkAdapterConfig, OfficialSdkAdapterError, RealFundsCanaryMarketCandidate,
    RealFundsCanaryMarketDiscovery, RealFundsCanaryMarketSelection, RealFundsCanaryReceipt,
    RealFundsCanaryRequest, select_real_funds_canary_market_with_diagnostics,
    validate_real_funds_canary_preconditions,
};

use super::shared::{authenticated_sdk_client, sdk_call_timeout, signer_from_env};

use anyhow::Context;
use polymarket_client_sdk_v2::clob::Client as SdkClient;
use polymarket_client_sdk_v2::clob::types::request::{OrderBookSummaryRequest, SpreadRequest};
use polymarket_client_sdk_v2::clob::types::{
    Amount as SdkAmount, OrderType as SdkOrderType, Side as SdkSide,
};
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};
use std::{cmp::Ordering, str::FromStr};
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
    let notional_usd = SdkDecimal::from_str(&request.market.notional_usd).map_err(|e| {
        OfficialSdkAdapterError::InvalidInput(format!("invalid canary notional_usd: {e}"))
    })?;

    let signable = time::timeout(
        timeout,
        client
            .market_order()
            .token_id(token_id)
            .price(price)
            .amount(SdkAmount::usdc(notional_usd).map_err(|e| {
                OfficialSdkAdapterError::InvalidInput(format!("invalid canary USDC amount: {e}"))
            })?)
            .side(SdkSide::Buy)
            .order_type(SdkOrderType::FOK)
            .build(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!("official SDK canary market order build timed out after {timeout:?}")
    })?
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

pub async fn discover_real_funds_canary_market(
    config: &OfficialSdkAdapterConfig,
    max_notional_usd: &str,
) -> anyhow::Result<RealFundsCanaryMarketSelection> {
    let discovery =
        discover_real_funds_canary_market_with_diagnostics(config, max_notional_usd).await?;
    discovery.selection.ok_or_else(|| {
        OfficialSdkAdapterError::SafetyGate(
            "no high-liquidity market candidate satisfied real funds canary constraints".into(),
        )
        .into()
    })
}

pub async fn discover_real_funds_canary_market_with_diagnostics(
    config: &OfficialSdkAdapterConfig,
    max_notional_usd: &str,
) -> anyhow::Result<RealFundsCanaryMarketDiscovery> {
    let client = SdkClient::new(
        &config.clob_host,
        polymarket_client_sdk_v2::clob::Config::builder()
            .use_server_time(true)
            .build(),
    )
    .context("creating public official SDK client for real-funds canary market discovery")?;
    let timeout = sdk_call_timeout();
    let markets = time::timeout(timeout, client.simplified_markets(None))
        .await
        .map_err(|_| {
            anyhow::anyhow!("official SDK simplified_markets() timed out after {timeout:?}")
        })?
        .context("official SDK simplified_markets() failed")?;

    let mut candidates = Vec::new();
    for market in markets.data.iter().filter(|market| {
        market.active && !market.closed && !market.archived && market.accepting_orders
    }) {
        let Some(condition_id) = market.condition_id.as_ref() else {
            continue;
        };
        for token in &market.tokens {
            let order_book = time::timeout(
                timeout,
                client.order_book(
                    &OrderBookSummaryRequest::builder()
                        .token_id(token.token_id)
                        .build(),
                ),
            )
            .await
            .map_err(|_| anyhow::anyhow!("official SDK order_book() timed out after {timeout:?}"))?
            .context("official SDK order_book() failed during canary market discovery")?;
            let spread = time::timeout(
                timeout,
                client.spread(&SpreadRequest::builder().token_id(token.token_id).build()),
            )
            .await
            .map_err(|_| anyhow::anyhow!("official SDK spread() timed out after {timeout:?}"))?
            .context("official SDK spread() failed during canary market discovery")?;

            let Some(best_ask) = order_book.asks.iter().min_by(|left, right| {
                decimal_for_sort(&left.price.to_string())
                    .partial_cmp(&decimal_for_sort(&right.price.to_string()))
                    .unwrap_or(Ordering::Equal)
            }) else {
                continue;
            };
            let spread_bps = decimal_to_bps(&spread.spread.to_string()).unwrap_or(u64::MAX);
            let liquidity_score = decimal_scaled_u64(&best_ask.size.to_string()).unwrap_or(0);
            candidates.push(RealFundsCanaryMarketCandidate {
                market_id: condition_id.to_string(),
                token_id: token.token_id.to_string(),
                active: market.active,
                accepting_orders: market.accepting_orders,
                closed: market.closed,
                archived: market.archived,
                best_ask: best_ask.price.to_string(),
                ask_size: best_ask.size.to_string(),
                spread_bps,
                min_order_size: order_book.min_order_size.to_string(),
                liquidity_score,
            });
        }
    }
    Ok(select_real_funds_canary_market_with_diagnostics(
        &candidates,
        max_notional_usd,
    ))
}

fn decimal_to_bps(value: &str) -> Option<u64> {
    let parsed = value.parse::<f64>().ok()?;
    parsed
        .is_finite()
        .then_some((parsed * 10_000.0).round() as u64)
}

fn decimal_scaled_u64(value: &str) -> Option<u64> {
    let parsed = value.parse::<f64>().ok()?;
    parsed
        .is_finite()
        .then_some((parsed * 1_000_000.0).round() as u64)
}

fn decimal_for_sort(value: &str) -> f64 {
    value.parse::<f64>().unwrap_or(f64::INFINITY)
}
