use super::InMemoryStore;
use async_trait::async_trait;
use chrono::Utc;
use pmx_core::{lifecycle_requires_reconcile, transition_order_state};

use crate::{
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore,
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, StoreError,
};

#[async_trait]
impl OrderLifecycleStore for InMemoryStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        let mut stored = order.clone();
        let now = Utc::now();
        if stored.created_at.is_none() {
            stored.created_at = Some(now);
        }
        stored.updated_at = Some(now);
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .insert(stored.order_id.clone(), stored);
        Ok(())
    }

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let Some(order) = state.orders.get_mut(&event.order_id) else {
            return Err(StoreError::NotFound(format!("order_id={}", event.order_id)));
        };
        let next = transition_order_state(order.lifecycle_state.clone(), event.event.clone())
            .map_err(|err| StoreError::Conflict(err.to_string()))?;
        order.lifecycle_state = next;
        order.updated_at = Some(Utc::now());
        let updated = order.clone();
        state.order_event_counter += 1;
        let mut stored_event = event.clone();
        stored_event.event_id = Some(state.order_event_counter);
        stored_event.created_at = Some(Utc::now());
        state.order_events.push(stored_event);
        Ok(updated)
    }

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .get(order_id)
            .cloned())
    }

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .order_events
            .iter()
            .filter(|event| event.order_id == query.order_id)
            .filter(|event| {
                query
                    .before_event_id
                    .map(|before| event.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.event_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl OrderReconcileBacklogStore for InMemoryStore {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
        let mut orders: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .orders
            .values()
            .filter(|order| order.account_id == query.account_id)
            .filter(|order| lifecycle_requires_reconcile(&order.lifecycle_state))
            .cloned()
            .collect();
        orders.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.order_id.cmp(&right.order_id))
        });
        orders.truncate(query.bounded_limit());
        Ok(orders)
    }
}
