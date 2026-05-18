use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord, StoreError,
    order_event_kind_from_str, order_lifecycle_state_from_str,
};

pub(super) async fn load_order_lifecycle(
    store: &PostgresStore,
    order_id: &str,
) -> Result<Option<OrderLifecycleRecord>, StoreError> {
    let client = store.client().await?;
    let row = client
        .query_opt(
            "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
             FROM orders
             WHERE order_id = $1",
            &[&order_id],
        )
        .await
        .map_err(map_db_error)?;
    row.map(|row| {
        let state: String = row.get(6);
        Ok(OrderLifecycleRecord {
            order_id: row.get(0),
            execution_id: row.get(1),
            account_id: row.get(2),
            condition_id: row.get(3),
            token_id: row.get(4),
            side: row.get(5),
            lifecycle_state: order_lifecycle_state_from_str(&state)?,
            remote_order_id: row.get(7),
            remote_state: row.get(8),
            created_at: Some(row.get(9)),
            updated_at: Some(row.get(10)),
        })
    })
    .transpose()
}

pub(super) async fn list_order_lifecycle_events(
    store: &PostgresStore,
    query: &OrderLifecycleEventQuery,
) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
    let client = store.client().await?;
    let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
    let rows = client
        .query(
            "SELECT event_id, order_id, event_type, event_source, correlation_id, payload, created_at
             FROM order_events
             WHERE order_id = $1
               AND ($2::bigint IS NULL OR event_id < $2)
             ORDER BY event_id DESC
             LIMIT $3",
            &[&query.order_id, &query.before_event_id, &bounded_limit],
        )
        .await
        .map_err(map_db_error)?;
    let mut events: Vec<OrderLifecycleEventRecord> = rows
        .into_iter()
        .map(|row| {
            let event_type: String = row.get(2);
            Ok(OrderLifecycleEventRecord {
                event_id: Some(row.get(0)),
                order_id: row.get(1),
                event: order_event_kind_from_str(&event_type)?,
                event_source: row.get(3),
                correlation_id: row.get(4),
                payload: row.get(5),
                created_at: Some(row.get(6)),
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    events.reverse();
    Ok(events)
}
