use pmx_core::{AccountId, CancelState, OrderEventKind, RemoteOrderId};
use pmx_gateway::ClobGateway;
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore};

use crate::order_lifecycle::payload;
use crate::{LiveCancelCommand, ServiceError};

pub async fn record_non_live_cancel_request<S>(
    store: &S,
    account_id: &str,
    order_id: &str,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<OrderLifecycleRecord, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    if account_id.trim().is_empty() || order_id.trim().is_empty() || reason.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "account_id, order_id and reason must be non-empty".into(),
        ));
    }
    let existing = store
        .load_order_lifecycle(order_id)
        .await?
        .ok_or_else(|| pmx_store::StoreError::NotFound(format!("order_id={order_id}")))?;
    if existing.account_id != account_id {
        return Err(ServiceError::Conflict(
            "order_id does not belong to account_id".into(),
        ));
    }
    let updated = store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event: OrderEventKind::CancelRequested,
            event_source: "pmx-service".into(),
            correlation_id: correlation_id.clone(),
            payload: payload::cancel_requested_non_live(correlation_id.as_deref(), reason.len()),
            created_at: None,
        })
        .await?;
    Ok(updated)
}

pub async fn cancel_order_with_gateway<S, G>(
    store: &S,
    gateway: &G,
    command: LiveCancelCommand,
) -> Result<OrderLifecycleRecord, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
    G: ClobGateway,
{
    if command.account_id.trim().is_empty()
        || command.order_id.trim().is_empty()
        || command.reason.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, order_id and reason must be non-empty".into(),
        ));
    }
    let existing = store
        .load_order_lifecycle(&command.order_id)
        .await?
        .ok_or_else(|| pmx_store::StoreError::NotFound(format!("order_id={}", command.order_id)))?;
    if existing.account_id != command.account_id {
        return Err(ServiceError::Conflict(
            "order_id does not belong to account_id".into(),
        ));
    }
    let Some(remote_order_id) = existing.remote_order_id.clone() else {
        return Err(ServiceError::Conflict(
            "live cancel requires a remote_order_id".into(),
        ));
    };
    store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: command.order_id.clone(),
            event: OrderEventKind::CancelRequested,
            event_source: "pmx-service".into(),
            correlation_id: scoped_correlation_id(&command.correlation_id, "requested"),
            payload: serde_json::json!({
                "kind": "cancel_requested_live_gateway",
                "reason_len": command.reason.len(),
                "remote_order_id_present": true,
                "raw_signed_payload_logged": false,
                "raw_signed_order_exposed": false
            }),
            created_at: None,
        })
        .await?;
    match gateway
        .cancel_order(
            &AccountId(command.account_id.clone()),
            &RemoteOrderId(remote_order_id),
        )
        .await
    {
        Ok(CancelState::RemoteAccepted) | Ok(CancelState::ConfirmedCanceled) => store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: command.order_id,
                event: OrderEventKind::CancelRemoteAccepted,
                event_source: "pmx-service".into(),
                correlation_id: scoped_correlation_id(&command.correlation_id, "accepted"),
                payload: serde_json::json!({
                    "kind": "cancel_remote_accepted_live_gateway",
                    "raw_signed_payload_logged": false,
                    "raw_signed_order_exposed": false
                }),
                created_at: None,
            })
            .await
            .map_err(ServiceError::from),
        Ok(_) | Err(pmx_gateway::GatewayError::RemoteUnknown(_)) => store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: command.order_id,
                event: OrderEventKind::RemoteUnknown,
                event_source: "pmx-service".into(),
                correlation_id: scoped_correlation_id(&command.correlation_id, "unknown"),
                payload: serde_json::json!({
                    "kind": "cancel_remote_unknown_live_gateway",
                    "operator_required": true,
                    "raw_signed_payload_logged": false,
                    "raw_signed_order_exposed": false
                }),
                created_at: None,
            })
            .await
            .map_err(ServiceError::from),
        Err(err) => Err(ServiceError::Conflict(err.to_string())),
    }
}

fn scoped_correlation_id(base: &Option<String>, suffix: &str) -> Option<String> {
    base.as_ref().map(|value| format!("{value}:{suffix}"))
}
