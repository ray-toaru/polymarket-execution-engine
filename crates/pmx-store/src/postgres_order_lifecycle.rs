use async_trait::async_trait;

mod backlog;
mod read;
mod write;

use crate::postgres::PostgresStore;
use crate::{
    OrderLifecycleEventQuery, OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore,
    OrderReconcileBacklogQuery, OrderReconcileBacklogStore, StoreError,
};

#[async_trait]
impl OrderLifecycleStore for PostgresStore {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError> {
        write::upsert_order_lifecycle(self, order).await
    }

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError> {
        write::record_order_lifecycle_event(self, event).await
    }

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError> {
        read::load_order_lifecycle(self, order_id).await
    }

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError> {
        read::list_order_lifecycle_events(self, query).await
    }
}

#[async_trait]
impl OrderReconcileBacklogStore for PostgresStore {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError> {
        backlog::list_reconcile_backlog_orders(self, query).await
    }
}
