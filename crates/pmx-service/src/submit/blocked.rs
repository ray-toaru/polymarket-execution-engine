use super::*;

pub async fn blocked_submit_outcome<S>(
    store: &S,
    plan: &pmx_core::ExecutionPlanSummary,
    idempotency_key: &str,
    request_fingerprint: &str,
    submit_attempt: u32,
    executor_version: &str,
    contract_version: &str,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore + IdempotencyStore + ExecutionLifecycleStore + Send + Sync,
{
    let receipt = SubmitReceipt {
        execution_id: plan.execution_id.clone(),
        receipt_id: format!("receipt-blocked-{submit_attempt}-{}", Uuid::new_v4()),
        status: SubmitStatus::Blocked,
        executor_version: executor_version.to_owned(),
        contract_version: contract_version.to_owned(),
    };
    let response_json = serde_json::to_string(&receipt).map_err(|err| {
        ServiceError::Internal(format!("submit receipt serialization failed: {err}"))
    })?;
    let response_fingerprint = fingerprint::response_fingerprint(&receipt)?;
    store
        .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
            event_id: None,
            execution_id: plan.execution_id.clone(),
            account_id: plan.account_id.0.clone(),
            event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
            event_source: "pmx-service".into(),
            payload: serde_json::json!({
                "submit_attempt": submit_attempt,
                "plan_status": format!("{:?}", plan.status),
                "no_remote_side_effect": true,
                "reservation_written": false,
                "receipt_id": receipt.receipt_id.clone(),
            }),
            created_at: None,
        })
        .await?;
    store.record_submit_receipt(&receipt).await?;
    store
        .finish_submit_attempt(
            &plan.account_id.0,
            &plan.execution_id,
            idempotency_key,
            request_fingerprint,
            &response_fingerprint,
            &response_json,
        )
        .await?;
    Ok(SubmitOutcome::Accepted(receipt))
}
