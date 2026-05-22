use pmx_core::ExecutionPlanSummary;

use crate::StoreError;
use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};

pub(in crate::postgres_execution) async fn save_plan_summary(
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
             ON CONFLICT (execution_id) DO NOTHING",
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
    let existing: serde_json::Value = load_json_payload(
        &client,
        "execution_plans",
        "execution_id",
        &plan.execution_id,
        "summary_json",
    )
    .await?;
    if existing != payload {
        return Err(StoreError::Conflict(
            "execution plan is immutable and cannot be overwritten".into(),
        ));
    }
    Ok(())
}

pub(in crate::postgres_execution) async fn load_plan_summary(
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
