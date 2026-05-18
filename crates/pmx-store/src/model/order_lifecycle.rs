use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{OrderEventKind, OrderLifecycleState};
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLifecycleRecord {
    pub order_id: String,
    pub execution_id: String,
    pub account_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub side: String,
    pub lifecycle_state: OrderLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLifecycleEventRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    pub order_id: String,
    pub event: OrderEventKind,
    pub event_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecycleEventQuery {
    pub order_id: String,
    pub limit: usize,
    pub before_event_id: Option<i64>,
}

impl OrderLifecycleEventQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[async_trait]
pub trait OrderLifecycleStore: Send + Sync {
    async fn upsert_order_lifecycle(&self, order: &OrderLifecycleRecord) -> Result<(), StoreError>;

    async fn record_order_lifecycle_event(
        &self,
        event: &OrderLifecycleEventRecord,
    ) -> Result<OrderLifecycleRecord, StoreError>;

    async fn load_order_lifecycle(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderLifecycleRecord>, StoreError>;

    async fn list_order_lifecycle_events(
        &self,
        query: &OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderReconcileBacklogQuery {
    pub account_id: String,
    pub limit: usize,
}

impl OrderReconcileBacklogQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[async_trait]
pub trait OrderReconcileBacklogStore: Send + Sync {
    async fn list_reconcile_backlog_orders(
        &self,
        query: &OrderReconcileBacklogQuery,
    ) -> Result<Vec<OrderLifecycleRecord>, StoreError>;
}
