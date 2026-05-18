use pmx_core::{
    OrderEventKind, OrderLifecycleDivergence, RemoteOrderObservation,
    classify_order_lifecycle_divergence,
};
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore};

use crate::ServiceError;

pub async fn record_non_live_cancel_request<S>(
    store: &S,
    order_id: &str,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<Option<OrderLifecycleRecord>, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    if order_id.trim().is_empty() || reason.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "order_id and reason must be non-empty".into(),
        ));
    }
    if store.load_order_lifecycle(order_id).await?.is_none() {
        return Ok(None);
    }
    let updated = store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event: OrderEventKind::CancelRequested,
            event_source: "pmx-service".into(),
            correlation_id: correlation_id.clone(),
            payload: serde_json::json!({
                "kind": "cancel_requested_non_live",
                "correlation_id": correlation_id,
                "reason_len": reason.len(),
                "no_remote_side_effect": true,
            }),
            created_at: None,
        })
        .await?;
    Ok(Some(updated))
}

pub async fn record_non_live_reconcile_observation<S>(
    store: &S,
    order_id: &str,
    event: OrderEventKind,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<Option<OrderLifecycleRecord>, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    if order_id.trim().is_empty() || reason.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "order_id and reason must be non-empty".into(),
        ));
    }
    if !matches!(
        event,
        OrderEventKind::ReconcileOpen | OrderEventKind::ReconcileMissing
    ) {
        return Err(ServiceError::BadRequest(
            "reconcile observation must be ReconcileOpen or ReconcileMissing".into(),
        ));
    }
    if store.load_order_lifecycle(order_id).await?.is_none() {
        return Ok(None);
    }
    let updated = store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event,
            event_source: "pmx-service".into(),
            correlation_id: correlation_id.clone(),
            payload: serde_json::json!({
                "kind": "reconcile_observed_non_live",
                "correlation_id": correlation_id,
                "reason_len": reason.len(),
                "no_remote_side_effect": true,
            }),
            created_at: None,
        })
        .await?;
    Ok(Some(updated))
}

pub async fn reconcile_order_lifecycle_divergence<S>(
    store: &S,
    order_id: &str,
    account_id: Option<&str>,
    remote_observation: RemoteOrderObservation,
    reason: &str,
    correlation_id: Option<String>,
) -> Result<Option<(OrderLifecycleDivergence, Option<OrderLifecycleRecord>)>, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    if order_id.trim().is_empty() || reason.trim().is_empty() {
        return Err(ServiceError::BadRequest(
            "order_id and reason must be non-empty".into(),
        ));
    }
    let Some(order) = store.load_order_lifecycle(order_id).await? else {
        return Ok(None);
    };
    if let Some(account_id) = account_id
        && order.account_id != account_id
    {
        return Err(ServiceError::Conflict(
            "order lifecycle account_id does not match request".into(),
        ));
    }
    let divergence =
        classify_order_lifecycle_divergence(&order.lifecycle_state, remote_observation);
    let updated = if let Some(event) = divergence.event.clone() {
        Some(
            store
                .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                    event_id: None,
                    order_id: order_id.to_owned(),
                    event,
                    event_source: "pmx-service".into(),
                    correlation_id: correlation_id.clone(),
                    payload: serde_json::json!({
                        "kind": "order_lifecycle_divergence_non_live",
                        "correlation_id": correlation_id,
                        "operator_required": divergence.operator_required,
                        "reason_len": reason.len(),
                        "classification": format!("{:?}", divergence.kind),
                        "no_remote_side_effect": true,
                    }),
                    created_at: None,
                })
                .await?,
        )
    } else {
        None
    };
    Ok(Some((divergence, updated)))
}
