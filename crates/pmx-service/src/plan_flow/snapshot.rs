use chrono::Utc;
use pmx_core::{FeasibilitySnapshot, NormalizedIntent, canonical_json_sha256};
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
