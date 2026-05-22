use super::*;

pub struct BlockedSubmitRequest<'a> {
    pub plan: &'a pmx_core::ExecutionPlanSummary,
    pub idempotency_key: &'a str,
    pub request_fingerprint: &'a str,
    pub submit_attempt: u32,
    pub owner_token: &'a str,
    pub executor_version: &'a str,
    pub contract_version: &'a str,
}

pub async fn blocked_submit_outcome<S>(
    store: &S,
    req: BlockedSubmitRequest<'_>,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore + IdempotencyStore + ExecutionLifecycleStore + Send + Sync,
{
    let receipt = SubmitReceipt {
        execution_id: req.plan.execution_id.clone(),
        receipt_id: format!("receipt-blocked-{}-{}", req.submit_attempt, Uuid::new_v4()),
        status: SubmitStatus::Blocked,
        executor_version: req.executor_version.to_owned(),
        contract_version: req.contract_version.to_owned(),
    };
    let response_json = serde_json::to_string(&receipt).map_err(|err| {
        ServiceError::Internal(format!("submit receipt serialization failed: {err}"))
    })?;
    let response_fingerprint = fingerprint::response_fingerprint(&receipt)?;
    store
        .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
            event_id: None,
            execution_id: req.plan.execution_id.clone(),
            account_id: req.plan.account_id.0.clone(),
            event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
            event_source: "pmx-service".into(),
            payload: serde_json::json!({
                "submit_attempt": req.submit_attempt,
                "plan_status": format!("{:?}", req.plan.status),
                "no_remote_side_effect": true,
                "reservation_written": false,
                "receipt_id": receipt.receipt_id.clone(),
            }),
            created_at: None,
        })
        .await?;
    store.record_submit_receipt(&receipt).await?;
    store
        .finish_submit_attempt(pmx_store::FinishSubmitAttempt {
            account_id: &req.plan.account_id.0,
            execution_id: &req.plan.execution_id,
            idempotency_key: req.idempotency_key,
            request_fingerprint: req.request_fingerprint,
            owner_token: req.owner_token,
            response_fingerprint: &response_fingerprint,
            response_json: &response_json,
        })
        .await?;
    Ok(SubmitOutcome::Accepted(receipt))
}
