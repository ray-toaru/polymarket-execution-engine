use pmx_core::OrderEventKind;
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore};

use crate::ServiceError;
use crate::order_lifecycle::payload;

pub async fn prepare_non_live_replace<S>(
    store: &S,
    account_id: &str,
    order_id: &str,
    replacement_ref: &str,
    correlation_id: String,
) -> Result<OrderLifecycleRecord, ServiceError>
where
    S: OrderLifecycleStore + Send + Sync,
{
    if account_id.trim().is_empty()
        || order_id.trim().is_empty()
        || replacement_ref.trim().is_empty()
        || correlation_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, order_id, replacement_ref and correlation_id must be non-empty".into(),
        ));
    }
    if !replacement_ref.starts_with("replacement:sha256:") {
        return Err(ServiceError::BadRequest(
            "replacement_ref must be an opaque replacement:sha256 reference".into(),
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

    store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event: OrderEventKind::ReplaceRequested,
            event_source: "pmx-service".into(),
            correlation_id: Some(format!("{correlation_id}:requested")),
            payload: payload::replace_non_live(
                "replace_requested_non_live",
                &correlation_id,
                replacement_ref,
            ),
            created_at: None,
        })
        .await?;
    store
        .record_order_lifecycle_event(&OrderLifecycleEventRecord {
            event_id: None,
            order_id: order_id.to_owned(),
            event: OrderEventKind::ReplacementPrepared,
            event_source: "pmx-service".into(),
            correlation_id: Some(format!("{correlation_id}:prepared")),
            payload: payload::replace_non_live(
                "replacement_prepared_non_live",
                &correlation_id,
                replacement_ref,
            ),
            created_at: None,
        })
        .await
        .map_err(ServiceError::from)
}
