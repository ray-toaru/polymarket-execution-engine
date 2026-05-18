use pmx_core::NormalizedIntent;

use crate::StoreError;
use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};

pub(in crate::postgres_execution) async fn save_normalized_intent(
    store: &PostgresStore,
    intent: &NormalizedIntent,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    let payload =
        serde_json::to_value(intent).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    client
        .execute(
            "INSERT INTO normalized_intents (normalized_intent_id, intent_hash, account_id, payload) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (normalized_intent_id) DO UPDATE SET payload = EXCLUDED.payload",
            &[
                &intent.normalized_intent_id,
                &intent.intent_hash.0,
                &intent.account_id.0,
                &payload,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(in crate::postgres_execution) async fn load_normalized_intent(
    store: &PostgresStore,
    normalized_intent_id: &str,
) -> Result<NormalizedIntent, StoreError> {
    let client = store.client().await?;
    load_json_payload(
        &client,
        "normalized_intents",
        "normalized_intent_id",
        normalized_intent_id,
        "payload",
    )
    .await
}
