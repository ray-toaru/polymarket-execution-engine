use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{AccountId, KillSwitchScope, RuntimeStateSummary};
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
        self.key_for_capabilities(&self.required_capabilities)
    }

    pub fn state_scope_key(&self) -> String {
        self.key_for_capabilities(&[])
    }

    fn key_for_capabilities(&self, capabilities: &[String]) -> String {
        let mut required_capabilities = capabilities.to_vec();
        required_capabilities.sort();
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.account_id,
            self.condition_id,
            self.collateral_profile_id.as_deref().unwrap_or("<default>"),
            required_capabilities.join(",")
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanaryRuntimeTruthQuery {
    pub account_id: String,
    pub condition_id: String,
    pub collateral_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanaryRuntimeTruthBindings {
    pub kill_switch_open: bool,
    pub live_submit_gate_ready: bool,
    pub idempotency_lease_ready: bool,
    pub order_cancel_reconciliation_ready: bool,
    pub runtime_worker_healthy: Option<bool>,
    pub geoblock_allowed: Option<bool>,
    pub repository_reservation_exists: Option<bool>,
    pub idempotency_key_written: Option<bool>,
    pub reconcile_worker_healthy: Option<bool>,
    pub cancel_only_fallback_ready: Option<bool>,
    pub balance_allowance_checked: Option<bool>,
    pub evidence_refs: Vec<String>,
}

impl CanaryRuntimeTruthBindings {
    pub fn all_ready(&self) -> bool {
        self.kill_switch_open
            && self.live_submit_gate_ready
            && self.idempotency_lease_ready
            && self.order_cancel_reconciliation_ready
    }
}

#[async_trait]
pub trait CanaryRuntimeTruthStore: Send + Sync {
    async fn load_canary_runtime_truth(
        &self,
        query: &CanaryRuntimeTruthQuery,
    ) -> Result<CanaryRuntimeTruthBindings, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillSwitchStateChange {
    pub scope: KillSwitchScope,
    pub account_id: Option<AccountId>,
    pub enabled: bool,
    pub state_version: i64,
    pub effective_at: DateTime<Utc>,
}

#[async_trait]
pub trait RuntimeControlStore: Send + Sync {
    async fn set_account_kill_switch(
        &self,
        account_id: &AccountId,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError>;

    async fn set_global_kill_switch(
        &self,
        enabled: bool,
        reason: &str,
    ) -> Result<KillSwitchStateChange, StoreError>;
}
