use pmx_core::SubmitReceipt;

use crate::postgres::PostgresStore;
use crate::postgres_support::{load_json_payload, map_db_error};
use crate::{StoreError, submit_status_str};

pub(super) async fn record_submit_receipt(
    store: &PostgresStore,
    receipt: &SubmitReceipt,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    let payload =
        serde_json::to_value(receipt).map_err(|e| StoreError::InvalidData(e.to_string()))?;
    client
        .execute(
            "INSERT INTO submit_receipts (execution_id, receipt_id, status, executor_version, contract_version, response_json) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (execution_id) DO UPDATE SET receipt_id = EXCLUDED.receipt_id, status = EXCLUDED.status, response_json = EXCLUDED.response_json, updated_at = now()",
            &[
                &receipt.execution_id,
                &receipt.receipt_id,
                &submit_status_str(&receipt.status),
                &receipt.executor_version,
                &receipt.contract_version,
                &payload,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn load_submit_receipt(
    store: &PostgresStore,
    execution_id: &str,
) -> Result<SubmitReceipt, StoreError> {
    let client = store.client().await?;
    load_json_payload(
        &client,
        "submit_receipts",
        "execution_id",
        execution_id,
        "response_json",
    )
    .await
}
