use crate::{
    OfficialSdkAdapterConfig, OfficialSdkAdapterError, RealFundsCanaryMarketCandidate,
    RealFundsCanaryMarketSelection, RealFundsCanaryMarketValidation, RealFundsCanaryReceipt,
    RealFundsCanaryRequest, RealFundsCanaryStageReport,
    select_real_funds_canary_market_with_diagnostics, validate_real_funds_canary_preconditions,
};

use super::shared::{authenticated_sdk_client, sdk_call_timeout, signer_from_env};

use anyhow::Context;
use polymarket_client_sdk_v2::clob::Client as SdkClient;
use polymarket_client_sdk_v2::clob::types::request::{OrderBookSummaryRequest, SpreadRequest};
use polymarket_client_sdk_v2::clob::types::{OrderType as SdkOrderType, Side as SdkSide};
use polymarket_client_sdk_v2::types::{Decimal as SdkDecimal, U256 as SdkU256};
use std::{cmp::Ordering, str::FromStr};
use tokio::time;

pub async fn run_real_funds_canary_gtc_post_only_cancel(
    config: &OfficialSdkAdapterConfig,
    request: RealFundsCanaryRequest,
) -> anyhow::Result<RealFundsCanaryReceipt> {
    run_real_funds_canary_gtc_post_only_cancel_with_reporter(config, request, |_| Ok(())).await
}

pub async fn run_real_funds_canary_gtc_post_only_cancel_with_reporter<F>(
    config: &OfficialSdkAdapterConfig,
    request: RealFundsCanaryRequest,
    mut report_stage: F,
) -> anyhow::Result<RealFundsCanaryReceipt>
where
    F: FnMut(&RealFundsCanaryStageReport) -> anyhow::Result<()>,
{
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
            .order_type(SdkOrderType::GTC)
            .post_only(true)
            .build(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!("official SDK canary limit order build timed out after {timeout:?}")
    })?
    .context("official SDK canary limit order build failed")?;

    let signed = time::timeout(timeout, client.sign(&signer, signable))
        .await
        .map_err(|_| anyhow::anyhow!("official SDK canary order sign timed out after {timeout:?}"))?
        .context("official SDK canary order sign failed")?;

    let response = match time::timeout(timeout, client.post_order(signed)).await {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            let summary = format!("official SDK canary post_order failed: {err}");
            report_stage(&RealFundsCanaryStageReport::operator_required(
                &request,
                "post_unknown",
                None,
                None,
                summary.clone(),
            ))?;
            return Err(anyhow::anyhow!(summary));
        }
        Err(_) => {
            let summary = format!("official SDK post_order timed out after {timeout:?}");
            report_stage(&RealFundsCanaryStageReport::operator_required(
                &request,
                "post_unknown",
                None,
                None,
                summary.clone(),
            ))?;
            return Err(anyhow::anyhow!(summary));
        }
    };

    let remote_status = format!("{:?}", response.status);
    let filled_or_matched = remote_status == "Matched";
    if filled_or_matched {
        report_stage(&RealFundsCanaryStageReport::operator_required(
            &request,
            "post_matched",
            Some(response.order_id.clone()),
            Some(remote_status.clone()),
            "GTC post-only canary order unexpectedly matched",
        ))?;
        return Err(OfficialSdkAdapterError::SafetyGate(
            "GTC post-only canary order unexpectedly matched".into(),
        )
        .into());
    }
    let order_id = response.order_id;
    if !response.success || order_id.is_empty() {
        report_stage(&RealFundsCanaryStageReport::operator_required(
            &request,
            "post_rejected",
            None,
            Some(remote_status.clone()),
            "GTC post-only canary post_order did not return an accepted order id",
        ))?;
        return Err(OfficialSdkAdapterError::SafetyGate(
            "GTC post-only canary post_order did not return an accepted order id".into(),
        )
        .into());
    }
    let post_accepted_report_error = report_stage(&RealFundsCanaryStageReport::stage(
        &request,
        "post_accepted",
        Some(order_id.clone()),
        Some(remote_status.clone()),
        true,
        filled_or_matched,
        false,
    ))
    .err();

    let cancel = match time::timeout(timeout, client.cancel_order(&order_id)).await {
        Ok(Ok(cancel)) => cancel,
        Ok(Err(err)) => {
            let summary = format!("official SDK canary cancel_order failed: {err}");
            report_stage(&RealFundsCanaryStageReport::operator_required(
                &request,
                "cancel_unknown",
                Some(order_id.clone()),
                Some(remote_status.clone()),
                summary.clone(),
            ))?;
            return Err(anyhow::anyhow!(summary));
        }
        Err(_) => {
            let summary = format!("official SDK cancel_order timed out after {timeout:?}");
            report_stage(&RealFundsCanaryStageReport::operator_required(
                &request,
                "cancel_unknown",
                Some(order_id.clone()),
                Some(remote_status.clone()),
                summary.clone(),
            ))?;
            return Err(anyhow::anyhow!(summary));
        }
    };
    let cancelled = cancel.canceled.iter().any(|canceled| canceled == &order_id)
        && !cancel.not_canceled.contains_key(&order_id);
    if !cancelled {
        report_stage(&RealFundsCanaryStageReport::operator_required(
            &request,
            "cancel_failed",
            Some(order_id.clone()),
            Some(remote_status.clone()),
            "GTC post-only canary order was posted but cancel confirmation failed",
        ))?;
        return Err(OfficialSdkAdapterError::SafetyGate(
            "GTC post-only canary order was posted but cancel confirmation failed".into(),
        )
        .into());
    }
    if let Some(err) = post_accepted_report_error {
        return Err(anyhow::anyhow!(
            "GTC post-only canary order was cancelled, but post_accepted report persistence failed: {err}"
        ));
    }

    let receipt = RealFundsCanaryReceipt {
        account_id: request.account_id,
        execution_id: request.execution_id,
        plan_hash: request.plan_hash,
        approval_hash: request.approval.approval_hash,
        market_candidate_sha256: request.market_candidate_sha256,
        idempotency_key: request.idempotency_key,
        remote_order_id: Some(order_id),
        remote_status,
        posted: true,
        filled_or_matched,
        cancelled,
        remote_side_effects: true,
        raw_signed_order_exposed: false,
    };
    Ok(receipt)
}

pub async fn validate_real_funds_canary_market(
    config: &OfficialSdkAdapterConfig,
    max_notional_usd: &str,
    candidate: RealFundsCanaryMarketCandidate,
) -> anyhow::Result<RealFundsCanaryMarketSelection> {
    let validation =
        validate_real_funds_canary_market_with_diagnostics(config, max_notional_usd, candidate)
            .await?;
    validation.selection.ok_or_else(|| {
        OfficialSdkAdapterError::SafetyGate(
            "provided market candidate did not satisfy real funds canary constraints".into(),
        )
        .into()
    })
}

pub async fn validate_real_funds_canary_market_with_diagnostics(
    config: &OfficialSdkAdapterConfig,
    max_notional_usd: &str,
    candidate: RealFundsCanaryMarketCandidate,
) -> anyhow::Result<RealFundsCanaryMarketValidation> {
    let client = SdkClient::new(
        &config.clob_host,
        polymarket_client_sdk_v2::clob::Config::builder()
            .use_server_time(true)
            .build(),
    )
    .context("creating public official SDK client for real-funds canary market validation")?;
    let timeout = sdk_call_timeout();

    let token_id = SdkU256::from_str(&candidate.token_id)
        .map_err(|e| OfficialSdkAdapterError::InvalidInput(format!("invalid token_id: {e}")))?;
    let order_book = time::timeout(
        timeout,
        client.order_book(
            &OrderBookSummaryRequest::builder()
                .token_id(token_id)
                .build(),
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK order_book() timed out after {timeout:?}"))?
    .context("official SDK order_book() failed during canary market validation")?;
    let spread = time::timeout(
        timeout,
        client.spread(&SpreadRequest::builder().token_id(token_id).build()),
    )
    .await
    .map_err(|_| anyhow::anyhow!("official SDK spread() timed out after {timeout:?}"))?
    .context("official SDK spread() failed during canary market validation")?;

    let refreshed_candidate = RealFundsCanaryMarketCandidate {
        best_ask: order_book
            .asks
            .iter()
            .min_by(|left, right| {
                decimal_for_sort(&left.price.to_string())
                    .partial_cmp(&decimal_for_sort(&right.price.to_string()))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|ask| ask.price.to_string())
            .unwrap_or_default(),
        limit_price: candidate.limit_price,
        ask_size: order_book
            .asks
            .iter()
            .min_by(|left, right| {
                decimal_for_sort(&left.price.to_string())
                    .partial_cmp(&decimal_for_sort(&right.price.to_string()))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|ask| ask.size.to_string())
            .unwrap_or_default(),
        spread_bps: decimal_to_bps(&spread.spread.to_string()).unwrap_or(u64::MAX),
        min_order_size: order_book.min_order_size.to_string(),
        liquidity_score: order_book
            .asks
            .iter()
            .map(|ask| decimal_scaled_u64(&ask.size.to_string()).unwrap_or(0))
            .max()
            .unwrap_or(0),
        ..candidate
    };

    Ok(select_real_funds_canary_market_with_diagnostics(
        &[refreshed_candidate],
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
