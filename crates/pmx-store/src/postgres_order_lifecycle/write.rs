use chrono::Utc;
use pmx_core::transition_order_state;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    OrderLifecycleEventRecord, OrderLifecycleRecord, StoreError, advisory_lock_key,
    order_event_kind_to_str, order_lifecycle_state_from_str, order_lifecycle_state_to_str,
};

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
    let row = match client
        .query_opt(
            "SELECT order_id, execution_id, account_id, condition_id, token_id, side, lifecycle_state, remote_order_id, remote_state, created_at, updated_at
             FROM orders
             WHERE order_id = $1",
            &[&event.order_id],
        )
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            PostgresStore::rollback(&client).await;
            return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
        }
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(map_db_error(err));
        }
    };
    let current_state: String = row.get(6);
    let current = order_lifecycle_state_from_str(&current_state)?;
    let event_type = order_event_kind_to_str(&event.event);
    if let Some(correlation_id) = event.correlation_id.as_deref() {
        let replay = match client
            .query_opt(
                "SELECT event_type FROM order_events
                 WHERE order_id = $1 AND correlation_id = $2
                 ORDER BY event_id ASC
                 LIMIT 1",
                &[&event.order_id, &correlation_id],
            )
            .await
        {
            Ok(row) => row,
            Err(err) => {
                PostgresStore::rollback(&client).await;
                return Err(map_db_error(err));
            }
        };
        if let Some(replay) = replay {
            let previous_event: String = replay.get(0);
            if previous_event == event_type {
                client.batch_execute("COMMIT").await.map_err(map_db_error)?;
                return Ok(OrderLifecycleRecord {
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
                });
            }
            PostgresStore::rollback(&client).await;
            return Err(StoreError::Conflict(
                "order lifecycle correlation_id reused with different event".into(),
            ));
        }
    }
    let next = match transition_order_state(current, event.event.clone()) {
        Ok(next) => next,
        Err(err) => {
            PostgresStore::rollback(&client).await;
            return Err(StoreError::Conflict(err.to_string()));
        }
    };
    let payload = event.payload.clone();
    if let Err(err) = client
        .execute(
            "INSERT INTO order_events (order_id, event_type, event_source, correlation_id, payload) VALUES ($1, $2, $3, $4, $5)",
            &[&event.order_id, &event_type, &event.event_source, &event.correlation_id, &payload],
        )
        .await
    {
        PostgresStore::rollback(&client).await;
        return Err(map_db_error(err));
    }
    if let Err(err) = client
        .execute(
            "UPDATE orders SET lifecycle_state = $2, updated_at = now() WHERE order_id = $1",
            &[&event.order_id, &order_lifecycle_state_to_str(&next)],
        )
        .await
    {
        PostgresStore::rollback(&client).await;
        return Err(map_db_error(err));
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
