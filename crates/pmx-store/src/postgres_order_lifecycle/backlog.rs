use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    OrderLifecycleRecord, OrderReconcileBacklogQuery, StoreError, order_lifecycle_state_from_str,
};

pub(super) async fn list_reconcile_backlog_orders(
    store: &PostgresStore,
    query: &OrderReconcileBacklogQuery,
) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
    let client = store.client().await?;
    let bounded_limit = i64::try_from(query.bounded_limit()).unwrap_or(500);
    let rows = client
        .query(
            "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
             FROM orders
             WHERE account_id = $1
               AND lifecycle_state IN ('REMOTE_UNKNOWN', 'PARTIAL_REMOTE_UNKNOWN')
             ORDER BY updated_at DESC, order_id ASC
             LIMIT $2",
            &[&query.account_id, &bounded_limit],
        )
        .await
        .map_err(map_db_error)?;
    rows.into_iter()
        .map(|row| {
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
        .collect()
}
