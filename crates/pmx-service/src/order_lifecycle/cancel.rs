use pmx_core::OrderEventKind;
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
