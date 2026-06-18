use chrono::Utc;
use pmx_core::{
    CoreError, FeasibilitySnapshot, NormalizedIntent, QuantityBound, canonical_json_sha256,
};
use pmx_gateway::MarketDataReader;
use pmx_policy::{
    CAP_MARKET_BOOK_FUTURE_DATED, CAP_MARKET_BOOK_INSUFFICIENT_TOP_LIQUIDITY,
    CAP_MARKET_BOOK_QUANTITY_UNSUPPORTED, CAP_MARKET_BOOK_STALE, CAP_MARKET_BOOK_UNAVAILABLE,
};
use pmx_store::ExecutionStore;
use uuid::Uuid;

use crate::{RuntimeStateProvider, ServiceError, SnapshotHashInput};

pub async fn capture_snapshot<S, R>(
    store: &S,
    runtime_state_provider: &R,
    normalized: NormalizedIntent,
    correlation_id: Option<String>,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    S: ExecutionStore + Send + Sync,
    R: RuntimeStateProvider,
{
    store.save_normalized_intent(&normalized).await?;
    let snapshot = build_snapshot(runtime_state_provider, &normalized, correlation_id).await?;
    store.save_snapshot(&snapshot).await?;
    Ok(snapshot)
}

pub(crate) async fn build_snapshot<R>(
    runtime_state_provider: &R,
    normalized: &NormalizedIntent,
    correlation_id: Option<String>,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    R: RuntimeStateProvider,
{
    let snapshot_id = Uuid::new_v4().to_string();
    let runtime_state = runtime_state_provider
        .capture_runtime_state(normalized)
        .await;
    let captured_at = Utc::now();
    let hash_input = SnapshotHashInput {
        snapshot_id: &snapshot_id,
        normalized_intent_id: &normalized.normalized_intent_id,
        runtime_state: &runtime_state,
        captured_at,
    };
    let snapshot_hash = canonical_json_sha256(&hash_input)
        .map_err(|err| ServiceError::Internal(err.to_string()))?;
    Ok(FeasibilitySnapshot {
        snapshot_id,
        snapshot_hash,
        normalized_intent_id: normalized.normalized_intent_id.clone(),
        correlation_id: correlation_id.or_else(|| normalized.correlation_id.clone()),
        runtime_state,
        captured_at,
    })
}

pub async fn capture_snapshot_with_market_data<S, R, M>(
    store: &S,
    runtime_state_provider: &R,
    market_data_reader: &M,
    normalized: NormalizedIntent,
    now_ms: i64,
    correlation_id: Option<String>,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    S: ExecutionStore + Send + Sync,
    R: RuntimeStateProvider,
    M: MarketDataReader,
{
    store.save_normalized_intent(&normalized).await?;
    let snapshot = build_snapshot_with_market_data(
        runtime_state_provider,
        market_data_reader,
        &normalized,
        now_ms,
        correlation_id,
    )
    .await?;
    store.save_snapshot(&snapshot).await?;
    Ok(snapshot)
}

async fn build_snapshot_with_market_data<R, M>(
    runtime_state_provider: &R,
    market_data_reader: &M,
    normalized: &NormalizedIntent,
    now_ms: i64,
    correlation_id: Option<String>,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    R: RuntimeStateProvider,
    M: MarketDataReader,
{
    let snapshot_id = Uuid::new_v4().to_string();
    let mut runtime_state = runtime_state_provider
        .capture_runtime_state(normalized)
        .await;
    match market_data_reader
        .read_market_book(&normalized.market.condition_id, &normalized.token_id)
        .await
    {
        Ok(book) => {
            if let Some(requested_shares) = requested_base_shares(normalized) {
                match book.has_top_liquidity_for(&normalized.side, requested_shares, now_ms) {
                    Ok(true) => {}
                    Ok(false) => runtime_state
                        .required_capabilities
                        .push(CAP_MARKET_BOOK_INSUFFICIENT_TOP_LIQUIDITY.into()),
                    Err(CoreError::StaleMarketData) => runtime_state
                        .required_capabilities
                        .push(CAP_MARKET_BOOK_STALE.into()),
                    Err(CoreError::FutureDatedMarketData) => runtime_state
                        .required_capabilities
                        .push(CAP_MARKET_BOOK_FUTURE_DATED.into()),
                    Err(_) => runtime_state
                        .required_capabilities
                        .push(CAP_MARKET_BOOK_UNAVAILABLE.into()),
                }
            } else {
                runtime_state
                    .required_capabilities
                    .push(CAP_MARKET_BOOK_QUANTITY_UNSUPPORTED.into());
            }
        }
        Err(_) => runtime_state
            .required_capabilities
            .push(CAP_MARKET_BOOK_UNAVAILABLE.into()),
    }
    let captured_at = Utc::now();
    let hash_input = SnapshotHashInput {
        snapshot_id: &snapshot_id,
        normalized_intent_id: &normalized.normalized_intent_id,
        runtime_state: &runtime_state,
        captured_at,
    };
    let snapshot_hash = canonical_json_sha256(&hash_input)
        .map_err(|err| ServiceError::Internal(err.to_string()))?;
    Ok(FeasibilitySnapshot {
        snapshot_id,
        snapshot_hash,
        normalized_intent_id: normalized.normalized_intent_id.clone(),
        correlation_id: correlation_id.or_else(|| normalized.correlation_id.clone()),
        runtime_state,
        captured_at,
    })
}

fn requested_base_shares(normalized: &NormalizedIntent) -> Option<&pmx_core::DecimalString> {
    match &normalized.quantity_bound {
        QuantityBound::WorstCaseBaseShares(shares) => Some(shares),
        QuantityBound::WorstCaseQuoteNotional(_) | QuantityBound::Unsupported(_) => None,
    }
}
