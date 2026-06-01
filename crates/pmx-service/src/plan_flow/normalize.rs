use pmx_core::{NormalizedIntent, TradeIntent, normalize_intent};
use pmx_store::ExecutionStore;

use crate::ServiceError;

pub async fn normalize<S>(
    store: &S,
    intent: TradeIntent,
    correlation_id: Option<String>,
) -> Result<NormalizedIntent, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let mut normalized =
        normalize_intent(intent).map_err(|err| ServiceError::BadRequest(err.to_string()))?;
    normalized.correlation_id = correlation_id;
    store.save_normalized_intent(&normalized).await?;
    Ok(normalized)
}
