use async_trait::async_trait;
use chrono::Utc;
use pmx_core::transition_order_state;

use crate::postgres::PostgresStore;
use crate::postgres_support::map_db_error;
use crate::{
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore,
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, StoreError, advisory_lock_key,
    order_event_kind_from_str, order_event_kind_to_str, order_lifecycle_state_from_str,
    order_lifecycle_state_to_str,
};

#[async_trait]
impl OrderLifecycleStore for PostgresStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        let client = self.client().await?;
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

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        let lock = advisory_lock_key("order_lifecycle", "order", &event.order_id);
        let client = self.client().await?;
        client.batch_execute("BEGIN").await.map_err(map_db_error)?;
        if let Err(err) = client
            .execute("SELECT pg_advisory_xact_lock($1)", &[&lock.0])
            .await
        {
            Self::rollback(&client).await;
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
                Self::rollback(&client).await;
                return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
            }
            Err(err) => {
                Self::rollback(&client).await;
                return Err(map_db_error(err));
            }
        };
        let current_state: String = row.get(6);
        let current = order_lifecycle_state_from_str(&current_state)?;
        let next = match transition_order_state(current, event.event.clone()) {
            Ok(next) => next,
            Err(err) => {
                Self::rollback(&client).await;
                return Err(StoreError::Conflict(err.to_string()));
            }
        };
        let payload = event.payload.clone();
        if let Err(err) = client
            .execute(
                "INSERT INTO order_events (order_id, event_type, event_source, correlation_id, payload) VALUES ($1, $2, $3, $4, $5)",
                &[&event.order_id, &order_event_kind_to_str(&event.event), &event.event_source, &event.correlation_id, &payload],
            )
            .await
        {
            Self::rollback(&client).await;
            return Err(map_db_error(err));
        }
        if let Err(err) = client
            .execute(
                "UPDATE orders SET lifecycle_state = $2, updated_at = now() WHERE order_id = $1",
                &[&event.order_id, &order_lifecycle_state_to_str(&next)],
            )
            .await
        {
            Self::rollback(&client).await;
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

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        let client = self.client().await?;
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

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        let client = self.client().await?;
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
}

#[async_trait]
impl OrderReconcileBacklogStore for PostgresStore {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
        let client = self.client().await?;
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
}
