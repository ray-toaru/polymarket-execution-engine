use pmx_core::OrderEventKind;
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore};

use crate::ServiceError;
use crate::order_lifecycle::payload;

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
