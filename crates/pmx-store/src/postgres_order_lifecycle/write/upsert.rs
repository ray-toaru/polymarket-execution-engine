use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{OrderLifecycleRecord, StoreError, order_lifecycle_state_to_str};

pub(super) async fn upsert_order_lifecycle(
    store: &PostgresStore,
    order: &OrderLifecycleRecord,
) -> Result<(), StoreError> {
    let client = store.client().await?;
    client
        .execute(
            "INSERT INTO orders \
             (order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now()) \
             ON CONFLICT (order_id) DO UPDATE SET \
               execution_id = EXCLUDED.execution_id, \
               account_id = EXCLUDED.account_id, \
               condition_id = EXCLUDED.condition_id, \
               token_id = EXCLUDED.token_id, \
               side = EXCLUDED.side, \
               lifecycle_state = EXCLUDED.lifecycle_state, \
               remote_order_id = EXCLUDED.remote_order_id, \
               remote_state = EXCLUDED.remote_state, \
               updated_at = now()",
            &[
                &order.order_id,
                &order.execution_id,
                &order.account_id,
                &order.condition_id,
                &order.token_id,
                &order.side,
                &order_lifecycle_state_to_str(&order.lifecycle_state),
                &order.remote_order_id,
                &order.remote_state,
            ],
        )
        .await
        .map_err(map_db_error)?;
    Ok(())
}
