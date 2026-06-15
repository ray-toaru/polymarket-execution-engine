use tokio_postgres::{Client, Row};

use crate::postgres_support::map_db_error;
use crate::{OrderLifecycleEventRecord, OrderLifecycleRecord, StoreError};

pub(super) async fn load_order_row(
    client: &Client,
    order_id: &str,
) -> Result<Option<Row>, StoreError> {
    client
        .query_opt(
            "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
             FROM orders
             WHERE order_id = $1",
            &[&order_id],
        )
        .await
        .map_err(map_db_error)
}

pub(super) async fn try_replay_existing_event(
    client: &Client,
    row: &Row,
    event: &OrderLifecycleEventRecord,
    event_type: &str,
    current: pmx_core::OrderLifecycleState,
) -> Result<Option<OrderLifecycleRecord>, StoreError> {
    let Some(correlation_id) = event.correlation_id.as_deref() else {
        return Ok(None);
    };
    let replay = client
        .query_opt(
            "SELECT event_type, event_source, payload FROM order_events
             WHERE order_id = $1 AND correlation_id = $2
             ORDER BY event_id ASC
             LIMIT 1",
            &[&event.order_id, &correlation_id],
        )
        .await
        .map_err(map_db_error)?;
    let Some(replay) = replay else {
        return Ok(None);
    };
    let previous_event: String = replay.get(0);
    let previous_source: String = replay.get(1);
    let previous_payload: serde_json::Value = replay.get(2);
    if previous_event != event_type
        || previous_source != event.event_source
        || previous_payload != event.payload
    {
        return Err(StoreError::Conflict(
            "order lifecycle correlation_id reused with different event payload".into(),
        ));
    }
    Ok(Some(OrderLifecycleRecord {
        order_id: row.get(0),
        execution_id: row.get(1),
        account_id: row.get(2),
        condition_id: row.get(3),
        token_id: row.get(4),
        side: row.get(5),
        lifecycle_state: current,
        remote_order_id: row.get(7),
        remote_state: row.get(8),
        created_at: Some(row.get(9)),
        updated_at: Some(row.get(10)),
    }))
}
