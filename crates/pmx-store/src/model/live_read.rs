use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{
    AccountId, LiveReadErrorCategory, LiveReadOperation, LiveReadOutcome, RemoteOrderId,
};
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveReadEventRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
    pub account_id: AccountId,
    pub operation: LiveReadOperation,
    pub outcome: LiveReadOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_order_id: Option<RemoteOrderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_category: Option<LiveReadErrorCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_error_summary: Option<String>,
    pub no_trading_side_effect: bool,
    pub redacted_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
}

impl From<pmx_core::LiveReadNormalizedEvent> for LiveReadEventRecord {
    fn from(event: pmx_core::LiveReadNormalizedEvent) -> Self {
        Self {
            event_id: None,
            account_id: event.account_id,
            operation: event.operation,
            outcome: event.outcome,
            remote_order_id: event.remote_order_id,
            remote_state: event.remote_state,
            error_category: event.error_category,
            redacted_error_summary: event.redacted_error_summary,
            no_trading_side_effect: event.no_trading_side_effect,
            redacted_fields: event.redacted_fields,
            observed_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReadEventQuery {
    pub limit: usize,
    pub before_event_id: Option<i64>,
    pub account_id: Option<AccountId>,
    pub operation: Option<LiveReadOperation>,
    pub outcome: Option<LiveReadOutcome>,
    pub remote_order_id: Option<RemoteOrderId>,
}

impl LiveReadEventQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

impl Default for LiveReadEventQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            before_event_id: None,
            account_id: None,
            operation: None,
            outcome: None,
            remote_order_id: None,
        }
    }
}

#[async_trait]
pub trait LiveReadEventStore: Send + Sync {
    async fn record_live_read_event(&self, event: &LiveReadEventRecord) -> Result<(), StoreError>;

    async fn list_live_read_events(
        &self,
        query: &LiveReadEventQuery,
    ) -> Result<Vec<LiveReadEventRecord>, StoreError>;
}
