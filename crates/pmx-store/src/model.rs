use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pmx_core::{
    ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot, NormalizedIntent,
    OrderEventKind, OrderLifecycleState, OrderReservation, RuntimeStateSummary,
    SignOnlyLifecycleRecord, SubmitReceipt,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database unavailable: {0}")]
    DatabaseUnavailable(String),
    #[error("serialization failure; retryable")]
    SerializationFailure,
    #[error("unexpected db data: {0}")]
    InvalidData(String),
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminAuditEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<i64>,
    pub principal_subject: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAuditQuery {
    pub limit: usize,
    pub before_audit_id: Option<i64>,
    pub operation: Option<String>,
    pub principal_subject: Option<String>,
    pub result: Option<String>,
    pub correlation_id: Option<String>,
}

impl AdminAuditQuery {
    pub fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

impl Default for AdminAuditQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            before_audit_id: None,
            operation: None,
            principal_subject: None,
            result: None,
            correlation_id: None,
        }
    }
}

#[async_trait]
pub trait AdminAuditStore: Send + Sync {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError>;

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError>;
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdvisoryLockKey(pub i64);

/// Deterministically maps a resource identity to a PostgreSQL advisory lock key.
pub fn advisory_lock_key(namespace: &str, account_id: &str, resource_key: &str) -> AdvisoryLockKey {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn feed(mut hash: u64, bytes: &[u8]) -> u64 {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    let mut hash = FNV_OFFSET;
    let parts = [
        namespace.as_bytes(),
        account_id.as_bytes(),
        resource_key.as_bytes(),
    ];
    for part in parts {
        hash = feed(hash, &(part.len() as u64).to_be_bytes());
        hash = feed(hash, part);
    }
    AdvisoryLockKey(i64::from_ne_bytes(hash.to_ne_bytes()))
}

#[async_trait]
pub trait ExecutionStore: Send + Sync {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError>;
    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError>;

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError>;
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError>;

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError>;
    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError>;

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError>;
    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError>;

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError>;
    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError>;
    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError>;
}

#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Begin or replay a submit request.
    ///
    /// Canonical identity is `(account_id, execution_id, idempotency_key)`.
    /// `submit_attempt` is executor-generated inside the transaction and is not supplied by the control plane.
    /// A different request fingerprint under the same identity must return `Conflict`.
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError>;

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyAction {
    /// This caller owns the in-progress side-effect slot and may continue.
    Proceed {
        submit_attempt: u32,
        owner_token: String,
    },
    /// Another caller already owns this idempotency identity and has not finished.
    /// Retrying callers must not sign/post remotely while this is fresh.
    InProgress {
        submit_attempt: u32,
        retry_after_ms: u64,
    },
    ReplayStoredResponse {
        response_fingerprint: String,
        response_json: String,
    },
    Conflict,
}
