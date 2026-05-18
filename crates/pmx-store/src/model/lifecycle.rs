use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::SignOnlyLifecycleRecord;
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLifecycleEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    pub execution_id: String,
    pub account_id: String,
    pub event_type: String,
    pub event_source: String,
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLifecycleQuery {
    pub execution_id: String,
    pub limit: usize,
    pub before_event_id: Option<i64>,
}

impl ExecutionLifecycleQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[async_trait]
pub trait ExecutionLifecycleStore: Send + Sync {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError>;

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignOnlyLifecycleQuery {
    pub execution_id: String,
    pub limit: usize,
    pub before_event_id: Option<i64>,
}

impl SignOnlyLifecycleQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[async_trait]
pub trait SignOnlyLifecycleStore: Send + Sync {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError>;

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError>;
}
