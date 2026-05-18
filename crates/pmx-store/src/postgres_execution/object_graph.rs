use pmx_core::{ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent};

use crate::StoreError;
use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};

pub(super) async fn save_normalized_intent(
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
            &[&intent.normalized_intent_id, &intent.intent_hash.0, &intent.account_id.0, &payload],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn load_normalized_intent(
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

pub(super) async fn save_snapshot(
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

pub(super) async fn load_snapshot(
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

pub(super) async fn save_decision(
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

pub(super) async fn load_decision(
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

pub(super) async fn save_plan_summary(
    store: &PostgresStore,
    plan: &ExecutionPlanSummary,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    let payload = serde_json::to_value(plan).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    client
        .execute(
            "INSERT INTO execution_plans \
             (execution_id, account_id, normalized_intent_id, snapshot_id, decision_id, plan_hash, status, summary_json) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (execution_id) DO UPDATE SET \
               account_id = EXCLUDED.account_id, \
               normalized_intent_id = EXCLUDED.normalized_intent_id, \
               snapshot_id = EXCLUDED.snapshot_id, \
               decision_id = EXCLUDED.decision_id, \
               plan_hash = EXCLUDED.plan_hash, \
               status = EXCLUDED.status, \
               summary_json = EXCLUDED.summary_json, \
               updated_at = now()",
            &[
                &plan.execution_id,
                &plan.account_id.0,
                &plan.normalized_intent_id,
                &plan.snapshot_id,
                &plan.decision_id,
                &plan.plan_hash.0,
                &format!("{:?}", plan.status).to_uppercase(),
                &payload,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn load_plan_summary(
    store: &PostgresStore,
    execution_id: &str,
) -> Result<ExecutionPlanSummary, StoreError> {
    let client = store.client().await?;
    load_json_payload(
        &client,
        "execution_plans",
        "execution_id",
        execution_id,
        "summary_json",
    )
    .await
}
