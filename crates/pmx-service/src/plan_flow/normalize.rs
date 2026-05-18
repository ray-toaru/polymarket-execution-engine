use pmx_core::{NormalizedIntent, TradeIntent, normalize_intent};
use pmx_store::ExecutionStore;

use crate::ServiceError;

pub async fn normalize<S>(store: &S, intent: TradeIntent) -> Result<NormalizedIntent, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized =
        normalize_intent(intent).map_err(|err| ServiceError::BadRequest(err.to_string()))?;
    store.save_normalized_intent(&normalized).await?;
    Ok(normalized)
}
