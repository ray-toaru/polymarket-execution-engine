use super::InMemoryStore;
use async_trait::async_trait;
use chrono::Utc;
use pmx_core::{lifecycle_requires_reconcile, transition_order_state};

use crate::{
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore,
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, StoreError,
};

#[path = "order_lifecycle/backlog.rs"]
mod backlog;

#[path = "order_lifecycle/query.rs"]
mod query;

#[path = "order_lifecycle/write.rs"]
mod write;

#[async_trait]
impl OrderLifecycleStore for InMemoryStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        write::upsert_order_lifecycle(self, order)
    }

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        write::record_order_lifecycle_event(self, event)
    }

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        write::load_order_lifecycle(self, order_id)
    }

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        query::list_order_lifecycle_events(self, query)
    }
}

#[async_trait]
impl OrderReconcileBacklogStore for InMemoryStore {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
        backlog::list_reconcile_backlog_orders(self, query)
    }
}
