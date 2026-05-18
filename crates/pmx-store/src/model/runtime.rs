use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::RuntimeStateSummary;
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerObservation {
    pub account_id: String,
    pub capability: String,
    pub worker_kind: String,
    pub status: String,
    pub should_fail_closed: bool,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait RuntimeWorkerObservationStore: Send + Sync {
    async fn record_runtime_worker_observation(
        &self,
        observation: &RuntimeWorkerObservation,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerHeartbeat {
    pub worker_id: String,
    pub role: String,
    pub capability: String,
    pub status: String,
    pub last_heartbeat_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[async_trait]
pub trait RuntimeWorkerHealthStore: Send + Sync {
    async fn record_worker_heartbeat(
        &self,
        heartbeat: &RuntimeWorkerHeartbeat,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkerStatusQuery {
    pub account_id: String,
    pub limit: usize,
    pub before_observed_at: Option<DateTime<Utc>>,
}

impl RuntimeWorkerStatusQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerStatusReport {
    pub heartbeats: Vec<RuntimeWorkerHeartbeat>,
    pub observations: Vec<RuntimeWorkerObservation>,
}

#[async_trait]
pub trait RuntimeWorkerStatusStore: Send + Sync {
    async fn list_runtime_worker_status(
        &self,
        query: &RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStateQuery {
    pub account_id: String,
    pub condition_id: String,
    pub collateral_profile_id: Option<String>,
    pub required_capabilities: Vec<String>,
}

impl RuntimeStateQuery {
    pub fn key(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}",
            self.account_id,
            self.condition_id,
            self.collateral_profile_id.as_deref().unwrap_or("<default>")
        )
    }
}

#[async_trait]
pub trait RuntimeStateStore: Send + Sync {
    /// Load the runtime state used to build a feasibility snapshot.
    ///
    /// Implementations must fail closed. Missing runtime rows or database errors must not produce
    /// an allow-like state; callers should receive Unknown/Error/Stale style fields instead.
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError>;
}
