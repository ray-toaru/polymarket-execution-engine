use super::*;
use pmx_core::{
    CollateralProfileStatus, GeoblockStatus, OrderEventKind, OrderLifecycleState, QuantityBound,
    RuntimeStateSummary, WorkerStatus,
};
use pmx_gateway::{GatewayError, PlanOrder};
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord};

pub struct LiveSubmitRequest<'a> {
    pub plan: &'a pmx_core::ExecutionPlanSummary,
    pub idempotency_key: &'a str,
    pub request_fingerprint: &'a str,
    pub submit_attempt: u32,
    pub owner_token: &'a str,
    pub executor_version: &'a str,
    pub contract_version: &'a str,
    pub correlation_id: Option<&'a str>,
}

pub async fn live_submit_outcome<S, R, P, G>(
    store: &S,
    runtime_state_provider: &R,
    signer_provider: &P,
    gateway: &G,
    req: LiveSubmitRequest<'_>,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore
        + IdempotencyStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + Send
        + Sync,
    R: RuntimeStateProvider,
    P: SignerProvider,
    G: ClobGateway,
{
    if !matches!(req.plan.status, pmx_core::PlanStatus::Ready) {
        return finish_live_receipt(
            store,
            &req,
            SubmitStatus::Blocked,
            "LIVE_SUBMIT_BLOCKED_PLAN_NOT_READY",
            serde_json::json!({"plan_status": format!("{:?}", req.plan.status)}),
        )
        .await;
    }

    let order = match plan_order(req.plan) {
        Ok(order) => order,
        Err(reason) => {
            return finish_live_receipt(
                store,
                &req,
                SubmitStatus::Blocked,
                "LIVE_SUBMIT_BLOCKED_PLAN_ORDER_UNSUPPORTED",
                serde_json::json!({"reason": reason, "remote_side_effect": false}),
            )
            .await;
        }
    };
    let normalized = match store
        .load_normalized_intent(&req.plan.normalized_intent_id)
        .await
    {
        Ok(normalized) => normalized,
        Err(_) => {
            return finish_live_receipt(
                store,
                &req,
                SubmitStatus::Blocked,
                "LIVE_SUBMIT_BLOCKED_MISSING_NORMALIZED_INTENT",
                serde_json::json!({"remote_side_effect": false}),
            )
            .await;
        }
    };
    let pre_sign_state = runtime_state_provider
        .capture_runtime_state(&normalized)
        .await;
    if let Some(reason) = runtime_submit_block_reason(&pre_sign_state) {
        return finish_live_receipt(
            store,
            &req,
            SubmitStatus::Blocked,
            "LIVE_SUBMIT_BLOCKED_PRE_SIGN_RUNTIME",
            serde_json::json!({"reason": reason, "remote_side_effect": false}),
        )
        .await;
    }

    let signer = match signer_provider
        .signer_for_account(&req.plan.account_id)
        .await
    {
        Ok(signer) => signer,
        Err(err) => {
            return finish_gateway_error(store, &req, "LIVE_SUBMIT_SIGNER_UNAVAILABLE", err).await;
        }
    };
    let signed = match signer.sign_order(&order).await {
        Ok(signed) => signed,
        Err(err) => {
            return finish_gateway_error(store, &req, "LIVE_SUBMIT_SIGNING_FAILED", err).await;
        }
    };
    let order_id = signed.internal_order_id.0.clone();
    store
        .upsert_order_lifecycle(&OrderLifecycleRecord {
            order_id: order_id.clone(),
            execution_id: req.plan.execution_id.clone(),
            account_id: req.plan.account_id.0.clone(),
            condition_id: req.plan.condition_id.0.clone(),
            token_id: req.plan.token_id.0.clone(),
            side: format!("{:?}", req.plan.side),
            lifecycle_state: OrderLifecycleState::Planned,
            remote_order_id: None,
            remote_state: None,
            created_at: None,
            updated_at: None,
        })
        .await?;
    record_order_event(
        store,
        &order_id,
        OrderEventKind::Signed,
        scoped_correlation_id(req.correlation_id, "signed").as_deref(),
        serde_json::json!({
            "submit_attempt": req.submit_attempt,
            "signer_fingerprint": signed.signer_fingerprint,
            "raw_signed_payload_logged": false,
            "raw_signed_order_exposed": false,
        }),
    )
    .await?;

    let pre_post_state = runtime_state_provider
        .capture_runtime_state(&normalized)
        .await;
    if let Some(reason) = runtime_submit_block_reason(&pre_post_state) {
        return finish_live_receipt(
            store,
            &req,
            SubmitStatus::Blocked,
            "LIVE_SUBMIT_BLOCKED_PRE_POST_RUNTIME",
            serde_json::json!({"reason": reason, "order_id": order_id, "remote_side_effect": false}),
        )
        .await;
    }

    record_order_event(
        store,
        &order_id,
        OrderEventKind::PostRequested,
        scoped_correlation_id(req.correlation_id, "post_requested").as_deref(),
        serde_json::json!({"submit_attempt": req.submit_attempt}),
    )
    .await?;
    match gateway.post_order(&signed).await {
        Ok(ack) => {
            let remote_order_id = ack.remote_order_id.0.clone();
            record_order_event(
                store,
                &order_id,
                OrderEventKind::RemotePosted,
                scoped_correlation_id(req.correlation_id, "remote_posted").as_deref(),
                serde_json::json!({
                    "remote_order_id": remote_order_id,
                    "accepted_at_ms": ack.accepted_at_ms,
                }),
            )
            .await?;
            store
                .upsert_order_lifecycle(&OrderLifecycleRecord {
                    order_id: order_id.clone(),
                    execution_id: req.plan.execution_id.clone(),
                    account_id: req.plan.account_id.0.clone(),
                    condition_id: req.plan.condition_id.0.clone(),
                    token_id: req.plan.token_id.0.clone(),
                    side: format!("{:?}", req.plan.side),
                    lifecycle_state: OrderLifecycleState::Posted,
                    remote_order_id: Some(remote_order_id.clone()),
                    remote_state: Some("OPEN".into()),
                    created_at: None,
                    updated_at: None,
                })
                .await?;
            let post_ack_state = runtime_state_provider
                .capture_runtime_state(&normalized)
                .await;
            if let Some(reason) = runtime_submit_block_reason(&post_ack_state) {
                return finish_live_receipt(
                    store,
                    &req,
                    SubmitStatus::PartialRemoteUnknown,
                    "LIVE_SUBMIT_POST_ACK_RUNTIME_DEGRADED",
                    serde_json::json!({
                        "reason": reason,
                        "order_id": order_id,
                        "remote_order_id": remote_order_id,
                        "remote_side_effect": true,
                        "operator_required": true,
                    }),
                )
                .await;
            }
            finish_live_receipt(
                store,
                &req,
                SubmitStatus::Posted,
                "LIVE_SUBMIT_REMOTE_POSTED",
                serde_json::json!({"order_id": order_id}),
            )
            .await
        }
        Err(GatewayError::RemoteUnknown(reason)) => {
            record_order_event(
                store,
                &order_id,
                OrderEventKind::RemoteUnknown,
                scoped_correlation_id(req.correlation_id, "remote_unknown").as_deref(),
                serde_json::json!({"reason": reason, "operator_required": true}),
            )
            .await?;
            finish_live_receipt(
                store,
                &req,
                SubmitStatus::RemoteUnknown,
                "LIVE_SUBMIT_REMOTE_UNKNOWN",
                serde_json::json!({"order_id": order_id, "operator_required": true}),
            )
            .await
        }
        Err(err @ (GatewayError::RemoteRejected(_) | GatewayError::AuthenticationFailed)) => {
            record_order_event(
                store,
                &order_id,
                OrderEventKind::RemoteRejected,
                scoped_correlation_id(req.correlation_id, "remote_rejected").as_deref(),
                serde_json::json!({"error": err.to_string(), "operator_required": true}),
            )
            .await?;
            finish_gateway_error(store, &req, "LIVE_SUBMIT_GATEWAY_REJECTED", err).await
        }
        Err(err) => finish_gateway_error(store, &req, "LIVE_SUBMIT_GATEWAY_REJECTED", err).await,
    }
}

fn plan_order(plan: &pmx_core::ExecutionPlanSummary) -> Result<PlanOrder, String> {
    let size = match &plan.quantity_bound {
        QuantityBound::WorstCaseBaseShares(value) => value.0.clone(),
        QuantityBound::WorstCaseQuoteNotional(_) => {
            return Err(
                "LIVE submit requires base-share size; quote-notional conversion is not wired"
                    .into(),
            );
        }
        QuantityBound::Unsupported(reason) => {
            return Err(format!(
                "LIVE submit cannot use unsupported quantity bound: {reason}"
            ));
        }
    };
    Ok(PlanOrder {
        execution_id: plan.execution_id.clone(),
        account_id: plan.account_id.clone(),
        token_id: plan.token_id.clone(),
        side: format!("{:?}", plan.side),
        limit_price: plan.limit_price.0.clone(),
        size,
        time_in_force: format!("{:?}", plan.time_in_force),
    })
}

fn runtime_submit_block_reason(state: &RuntimeStateSummary) -> Option<&'static str> {
    if state.kill_switch_enabled {
        return Some("kill_switch_enabled");
    }
    if !matches!(state.geoblock_status, GeoblockStatus::Allowed) {
        return Some("geoblock_not_allowed");
    }
    if !matches!(state.worker_status, WorkerStatus::Healthy) {
        return Some("worker_not_healthy");
    }
    if !matches!(
        state.collateral_profile_status,
        CollateralProfileStatus::Resolved | CollateralProfileStatus::DefaultResolved
    ) {
        return Some("collateral_profile_not_resolved");
    }
    None
}

fn scoped_correlation_id(base: Option<&str>, suffix: &str) -> Option<String> {
    base.map(|value| format!("{value}:{suffix}"))
}

async fn record_order_event<S>(
    store: &S,
    order_id: &str,
    event: OrderEventKind,
    correlation_id: Option<&str>,
    payload: serde_json::Value,
) -> Result<(), ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event,
            event_source: "pmx-service".into(),
            correlation_id: correlation_id.map(str::to_owned),
            payload,
            created_at: None,
        })
        .await?;
    Ok(())
}

async fn finish_gateway_error<S>(
    store: &S,
    req: &LiveSubmitRequest<'_>,
    event_type: &str,
    err: GatewayError,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore + IdempotencyStore + ExecutionLifecycleStore + Send + Sync,
{
    let error = err.to_string();
    let status = match &err {
        GatewayError::Disabled | GatewayError::SigningUnavailable => SubmitStatus::Blocked,
        GatewayError::RemoteUnknown(_) => SubmitStatus::RemoteUnknown,
        GatewayError::RemoteRejected(_) | GatewayError::AuthenticationFailed => {
            SubmitStatus::Rejected
        }
    };
    finish_live_receipt(
        store,
        req,
        status,
        event_type,
        serde_json::json!({"error": error}),
    )
    .await
}

async fn finish_live_receipt<S>(
    store: &S,
    req: &LiveSubmitRequest<'_>,
    status: SubmitStatus,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<SubmitOutcome, ServiceError>
where
    S: ExecutionStore + IdempotencyStore + ExecutionLifecycleStore + Send + Sync,
{
    let receipt = SubmitReceipt {
        execution_id: req.plan.execution_id.clone(),
        receipt_id: format!("receipt-live-{}-{}", req.submit_attempt, Uuid::new_v4()),
        status,
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
            event_type: event_type.to_owned(),
            event_source: "pmx-service".into(),
            payload: match req.correlation_id {
                Some(correlation_id) => serde_json::json!({
                    "correlation_id": correlation_id,
                    "body": payload,
                }),
                None => payload,
            },
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
