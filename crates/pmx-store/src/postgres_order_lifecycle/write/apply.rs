use tokio_postgres::Client;

use crate::postgres_support::map_db_error;
use crate::{OrderLifecycleEventRecord, StoreError, order_lifecycle_state_to_str};

pub(super) async fn apply_order_lifecycle_event(
    client: &Client,
    event: &OrderLifecycleEventRecord,
    event_type: &str,
    next: &pmx_core::OrderLifecycleState,
) -> Result<(), StoreError> {
    let payload = event.payload.clone();
    client
        .execute(
            "INSERT INTO order_events (order_id, event_type, event_source, correlation_id, payload) VALUES ($1, $2, $3, $4, $5)",
            &[&event.order_id, &event_type, &event.event_source, &event.correlation_id, &payload],
        )
        .await
        .map_err(map_db_error)?;
    client
        .execute(
            "UPDATE orders SET lifecycle_state = $2, updated_at = now() WHERE order_id = $1",
            &[&event.order_id, &order_lifecycle_state_to_str(next)],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}
