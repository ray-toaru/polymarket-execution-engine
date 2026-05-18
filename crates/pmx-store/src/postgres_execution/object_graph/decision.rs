use pmx_core::ConstraintDecision;

use crate::StoreError;
use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};

pub(in crate::postgres_execution) async fn save_decision(
    store: &PostgresStore,
    decision: &ConstraintDecision,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    let payload =
        serde_json::to_value(decision).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    let reasons = serde_json::to_value(&decision.reasons)
        .map_err(|e| StoreError::InvalidData(e.to_string()))?;
    let snapshot_id: Option<String> = None;
    client
        .execute(
            "INSERT INTO constraint_decisions (decision_id, decision_hash, snapshot_id, status, reasons, payload) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (decision_id) DO UPDATE SET status = EXCLUDED.status, reasons = EXCLUDED.reasons, payload = EXCLUDED.payload",
            &[
                &decision.decision_id,
                &decision.decision_hash.0,
                &snapshot_id,
                &format!("{:?}", decision.status).to_uppercase(),
                &reasons,
                &payload,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(in crate::postgres_execution) async fn load_decision(
    store: &PostgresStore,
    decision_id: &str,
) -> Result<ConstraintDecision, StoreError> {
    let client = store.client().await?;
    load_json_payload(
        &client,
        "constraint_decisions",
        "decision_id",
        decision_id,
        "payload",
    )
    .await
}
