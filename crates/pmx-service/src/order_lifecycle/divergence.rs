use pmx_core::{
    OrderLifecycleDivergence, RemoteOrderObservation, classify_order_lifecycle_divergence,
};
use pmx_store::{OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore};

use crate::ServiceError;
use crate::order_lifecycle::payload;

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
                    payload: payload::order_lifecycle_divergence_non_live(
                        correlation_id.as_deref(),
                        divergence.operator_required,
                        reason.len(),
                        format!("{:?}", divergence.kind),
                    ),
                    created_at: None,
                })
                .await?,
        )
    } else {
        None
    };
    Ok(Some((divergence, updated)))
}
