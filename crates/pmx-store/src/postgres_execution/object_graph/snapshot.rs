use pmx_core::FeasibilitySnapshot;

use crate::StoreError;
use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};

pub(in crate::postgres_execution) async fn save_snapshot(
    store: &PostgresStore,
    snapshot: &FeasibilitySnapshot,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    let payload =
        serde_json::to_value(snapshot).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    client
        .execute(
            "INSERT INTO feasibility_snapshots (snapshot_id, snapshot_hash, normalized_intent_id, payload, captured_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (snapshot_id) DO UPDATE SET payload = EXCLUDED.payload",
            &[
                &snapshot.snapshot_id,
                &snapshot.snapshot_hash.0,
                &snapshot.normalized_intent_id,
                &payload,
                &snapshot.captured_at,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(in crate::postgres_execution) async fn load_snapshot(
    store: &PostgresStore,
    snapshot_id: &str,
) -> Result<FeasibilitySnapshot, StoreError> {
    let client = store.client().await?;
    load_json_payload(
        &client,
        "feasibility_snapshots",
        "snapshot_id",
        snapshot_id,
        "payload",
    )
    .await
}
