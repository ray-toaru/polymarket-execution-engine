#[path = "write/apply.rs"]
mod apply;

#[path = "write/replay.rs"]
mod replay;

#[path = "write/upsert.rs"]
mod upsert;

use chrono::Utc;
use pmx_core::transition_order_state;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    OrderLifecycleEventRecord, OrderLifecycleRecord, StoreError, advisory_lock_key,
    order_event_kind_to_str, order_lifecycle_state_from_str,
};

pub(super) async fn upsert_order_lifecycle(
    store: &PostgresStore,
    order: &OrderLifecycleRecord,
) -> Result<(), StoreError> {
    upsert::upsert_order_lifecycle(store, order).await
}

pub(super) async fn record_order_lifecycle_event(
    store: &PostgresStore,
    event: &OrderLifecycleEventRecord,
) -> Result<OrderLifecycleRecord, StoreError> {
    let lock = advisory_lock_key("order_lifecycle", "order", &event.order_id);
    let client = store.client().await?;
    client.batch_execute("BEGIN").await.map_err(map_db_error)?;
    if let Err(err) = client
        .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
        .await
    {
        PostgresStore::rollback(&client).await;
        return Err(map_db_error(err));
    }
    let row = match replay::load_order_row(&client, &event.order_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            PostgresStore::rollback(&client).await;
            return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(err);
        }
    };
    let current_state: String = row.get(6);
    let current = order_lifecycle_state_from_str(&current_state)?;
    let event_type = order_event_kind_to_str(&event.event);

    if let Some(replayed) =
        replay::try_replay_existing_event(&client, &row, event, event_type, current.clone()).await?
    {
        client.batch_execute("COMMIT").await.map_err(map_db_error)?;
        return Ok(replayed);
    }

    let next = match transition_order_state(current, event.event.clone()) {
        Ok(next) => next,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(StoreError::Conflict(err.to_string()));
        }
    };
    if let Err(err) = apply::apply_order_lifecycle_event(&client, event, event_type, &next).await {
        PostgresStore::rollback(&client).await;
        return Err(err);
    }
    client.batch_execute("COMMIT").await.map_err(map_db_error)?;
    Ok(OrderLifecycleRecord {
        order_id: row.get(0),
        execution_id: row.get(1),
        account_id: row.get(2),
        condition_id: row.get(3),
        token_id: row.get(4),
        side: row.get(5),
        lifecycle_state: next,
        remote_order_id: row.get(7),
        remote_state: row.get(8),
        created_at: Some(row.get(9)),
        updated_at: Some(Utc::now()),
    })
}
