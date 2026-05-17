use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use pmx_core::{
    CollateralProfileStatus, ConstraintDecision, ExecutionPlanSummary, FeasibilitySnapshot,
    GeoblockStatus, NormalizedIntent, OrderEventKind, OrderLifecycleState, OrderReservation,
    QuantityBound, ReservationState, RuntimeStateSummary, SignOnlyLifecycleRecord,
    SignOnlyLifecycleState, SubmitReceipt, SubmitStatus, WorkerStatus,
    sign_only_lifecycle_records_equivalent, transition_order_state, transition_sign_only_lifecycle,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub mod postgres;
pub use postgres::PostgresStore;

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

#[derive(Clone, Default)]
pub struct InMemoryStore {
    inner: Arc<Mutex<InMemoryState>>,
}

#[derive(Default)]
struct InMemoryState {
    normalized: HashMap<String, NormalizedIntent>,
    snapshots: HashMap<String, FeasibilitySnapshot>,
    decisions: HashMap<String, ConstraintDecision>,
    plans: HashMap<String, ExecutionPlanSummary>,
    reservations: HashMap<String, OrderReservation>,
    receipts: HashMap<String, SubmitReceipt>,
    idempotency: HashMap<String, IdempotencyRecord>,
    attempt_counters: HashMap<String, u32>,
    admin_audit: Vec<AdminAuditEvent>,
    admin_audit_counter: i64,
    runtime_states: HashMap<String, RuntimeStateSummary>,
    lifecycle_events: Vec<ExecutionLifecycleEvent>,
    lifecycle_event_counter: i64,
    sign_only_lifecycle_events: Vec<SignOnlyLifecycleRecord>,
    sign_only_event_counter: i64,
    runtime_worker_observations: Vec<RuntimeWorkerObservation>,
    worker_health: HashMap<String, RuntimeWorkerHeartbeat>,
    orders: HashMap<String, OrderLifecycleRecord>,
    order_events: Vec<OrderLifecycleEventRecord>,
    order_event_counter: i64,
}

#[derive(Clone)]
struct IdempotencyRecord {
    submit_attempt: u32,
    request_fingerprint: String,
    response_fingerprint: Option<String>,
    response_json: Option<String>,
}

impl InMemoryStore {
    pub fn set_runtime_state_for_test(
        &self,
        account_id: &str,
        condition_id: &str,
        collateral_profile_id: Option<&str>,
        runtime_state: RuntimeStateSummary,
    ) {
        let query = RuntimeStateQuery {
            account_id: account_id.to_owned(),
            condition_id: condition_id.to_owned(),
            collateral_profile_id: collateral_profile_id.map(ToOwned::to_owned),
            required_capabilities: vec![],
        };
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .insert(query.key(), runtime_state);
    }

    fn observations_for_account(&self, account_id: &str) -> Vec<RuntimeWorkerObservation> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .iter()
            .filter(|observation| {
                observation.account_id == account_id && runtime_observation_is_fresh(observation)
            })
            .cloned()
            .collect()
    }
}

pub const DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS: i64 = 120;
pub const RUNTIME_OBSERVATION_TTL_SECONDS: i64 = DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS;

/// Runtime observation freshness horizon.
///
/// The default is intentionally conservative and remains configurable because v0.23 has not yet
/// established a validated worker heartbeat cadence. Invalid or non-positive values fail closed
/// back to the default instead of silently extending freshness.
pub fn runtime_observation_ttl_seconds() -> i64 {
    std::env::var("PMX_RUNTIME_OBSERVATION_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0 && *value <= 86_400)
        .unwrap_or(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS)
}

fn runtime_observation_is_fresh(observation: &RuntimeWorkerObservation) -> bool {
    observation
        .observed_at
        .map(|observed_at| {
            observed_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
        })
        .unwrap_or(true)
}

fn runtime_worker_heartbeat_is_fresh(heartbeat: &RuntimeWorkerHeartbeat) -> bool {
    heartbeat.last_heartbeat_at >= Utc::now() - Duration::seconds(runtime_observation_ttl_seconds())
}

fn worker_status_from_heartbeats(
    heartbeats: &[RuntimeWorkerHeartbeat],
    required_capabilities: &[String],
) -> WorkerStatus {
    if required_capabilities.is_empty() {
        return WorkerStatus::Healthy;
    }
    let mut degraded = false;
    for capability in required_capabilities {
        let Some(heartbeat) = heartbeats
            .iter()
            .filter(|heartbeat| &heartbeat.capability == capability)
            .max_by_key(|heartbeat| heartbeat.last_heartbeat_at)
        else {
            return WorkerStatus::Unknown;
        };
        let normalized = heartbeat.status.trim().to_ascii_uppercase();
        if !runtime_worker_heartbeat_is_fresh(heartbeat)
            || matches!(normalized.as_str(), "STALE" | "ERROR" | "DOWN")
        {
            return WorkerStatus::Stale;
        }
        if normalized == "DEGRADED" {
            degraded = true;
        } else if normalized != "HEALTHY" {
            return WorkerStatus::Unknown;
        }
    }
    if degraded {
        WorkerStatus::Degraded
    } else {
        WorkerStatus::Healthy
    }
}

fn sanitize_admin_audit_event(mut event: AdminAuditEvent) -> AdminAuditEvent {
    event.audit_id = None;
    event.created_at = None;
    event
}

fn sanitize_execution_lifecycle_event(
    mut event: ExecutionLifecycleEvent,
) -> ExecutionLifecycleEvent {
    event.event_id = None;
    event.created_at = None;
    event
}

fn sanitize_sign_only_lifecycle_record(
    mut record: SignOnlyLifecycleRecord,
) -> SignOnlyLifecycleRecord {
    record.event_id = None;
    record.created_at = None;
    record
}

pub(crate) fn sign_only_lifecycle_record_is_replay(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<bool, StoreError> {
    if let Some(client_event_id) = record.client_event_id.as_deref() {
        if client_event_id.trim().is_empty() {
            return Err(StoreError::Conflict(
                "sign-only lifecycle client_event_id must not be empty".into(),
            ));
        }
        if let Some(previous) = existing
            .iter()
            .find(|candidate| candidate.client_event_id.as_deref() == Some(client_event_id))
        {
            if sign_only_lifecycle_records_equivalent(previous, record) {
                return Ok(true);
            }
            return Err(StoreError::Conflict(
                "sign-only lifecycle client_event_id reused with different event payload".into(),
            ));
        }
    }
    Ok(existing
        .last()
        .map(|last| sign_only_lifecycle_records_equivalent(last, record))
        .unwrap_or(false))
}

pub(crate) fn validate_sign_only_lifecycle_append_for_store(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<(), StoreError> {
    if !record.no_remote_side_effect {
        return Err(StoreError::Conflict(
            "sign-only lifecycle record must not contain remote side effects".into(),
        ));
    }
    if sign_only_lifecycle_record_is_replay(existing, record)? {
        return Ok(());
    }
    if let Some(first) = existing.first()
        && first.account_id != record.account_id
    {
        return Err(StoreError::Conflict(
            "sign-only lifecycle account_id does not match existing execution history".into(),
        ));
    }
    let from = existing
        .last()
        .map(|event| event.state.clone())
        .unwrap_or(SignOnlyLifecycleState::Planned);
    if matches!(
        from,
        SignOnlyLifecycleState::SignedDryRun
            | SignOnlyLifecycleState::Failed
            | SignOnlyLifecycleState::Abandoned
    ) {
        return Err(StoreError::Conflict(
            "sign-only lifecycle is already terminal".into(),
        ));
    }
    let expected = transition_sign_only_lifecycle(from.clone(), record.event.clone())
        .map_err(|err| StoreError::Conflict(err.to_string()))?;
    if expected != record.state {
        return Err(StoreError::Conflict(format!(
            "sign-only lifecycle state mismatch: event {:?} from {:?} yields {:?}, got {:?}",
            record.event, from, expected, record.state
        )));
    }
    match (&record.state, record.signed_order_ref.as_ref()) {
        (SignOnlyLifecycleState::SignedDryRun, Some(value)) if !value.trim().is_empty() => {}
        (SignOnlyLifecycleState::SignedDryRun, _) => {
            return Err(StoreError::Conflict(
                "SignedDryRun sign-only lifecycle record requires a non-empty signed_order_ref"
                    .into(),
            ));
        }
        (_, Some(_)) => {
            return Err(StoreError::Conflict(
                "signed_order_ref is only allowed for SignedDryRun sign-only lifecycle records"
                    .into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn order_lifecycle_state_to_str(state: &OrderLifecycleState) -> &'static str {
    match state {
        OrderLifecycleState::Planned => "PLANNED",
        OrderLifecycleState::Signed => "SIGNED",
        OrderLifecycleState::PostRequested => "POST_REQUESTED",
        OrderLifecycleState::Posted => "POSTED",
        OrderLifecycleState::PartiallyFilled => "PARTIALLY_FILLED",
        OrderLifecycleState::Filled => "FILLED",
        OrderLifecycleState::CancelRequested => "CANCEL_REQUESTED",
        OrderLifecycleState::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderLifecycleState::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderLifecycleState::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderLifecycleState::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        OrderLifecycleState::Failed => "FAILED",
    }
}

fn order_lifecycle_state_from_str(value: &str) -> Result<OrderLifecycleState, StoreError> {
    match value {
        "PLANNED" => Ok(OrderLifecycleState::Planned),
        "SIGNED" => Ok(OrderLifecycleState::Signed),
        "POST_REQUESTED" => Ok(OrderLifecycleState::PostRequested),
        "POSTED" => Ok(OrderLifecycleState::Posted),
        "PARTIALLY_FILLED" => Ok(OrderLifecycleState::PartiallyFilled),
        "FILLED" => Ok(OrderLifecycleState::Filled),
        "CANCEL_REQUESTED" => Ok(OrderLifecycleState::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderLifecycleState::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderLifecycleState::CancelConfirmed),
        "REMOTE_UNKNOWN" => Ok(OrderLifecycleState::RemoteUnknown),
        "PARTIAL_REMOTE_UNKNOWN" => Ok(OrderLifecycleState::PartialRemoteUnknown),
        "FAILED" => Ok(OrderLifecycleState::Failed),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle state: {other}"
        ))),
    }
}

fn order_event_kind_to_str(event: &OrderEventKind) -> &'static str {
    match event {
        OrderEventKind::Signed => "SIGNED",
        OrderEventKind::PostRequested => "POST_REQUESTED",
        OrderEventKind::RemotePosted => "REMOTE_POSTED",
        OrderEventKind::RemoteRejected => "REMOTE_REJECTED",
        OrderEventKind::RemoteUnknown => "REMOTE_UNKNOWN",
        OrderEventKind::PartialFill => "PARTIAL_FILL",
        OrderEventKind::FullFill => "FULL_FILL",
        OrderEventKind::CancelRequested => "CANCEL_REQUESTED",
        OrderEventKind::CancelRemoteAccepted => "CANCEL_REMOTE_ACCEPTED",
        OrderEventKind::CancelConfirmed => "CANCEL_CONFIRMED",
        OrderEventKind::ReconcileOpen => "RECONCILE_OPEN",
        OrderEventKind::ReconcileMissing => "RECONCILE_MISSING",
    }
}

fn order_event_kind_from_str(value: &str) -> Result<OrderEventKind, StoreError> {
    match value {
        "SIGNED" => Ok(OrderEventKind::Signed),
        "POST_REQUESTED" => Ok(OrderEventKind::PostRequested),
        "REMOTE_POSTED" => Ok(OrderEventKind::RemotePosted),
        "REMOTE_REJECTED" => Ok(OrderEventKind::RemoteRejected),
        "REMOTE_UNKNOWN" => Ok(OrderEventKind::RemoteUnknown),
        "PARTIAL_FILL" => Ok(OrderEventKind::PartialFill),
        "FULL_FILL" => Ok(OrderEventKind::FullFill),
        "CANCEL_REQUESTED" => Ok(OrderEventKind::CancelRequested),
        "CANCEL_REMOTE_ACCEPTED" => Ok(OrderEventKind::CancelRemoteAccepted),
        "CANCEL_CONFIRMED" => Ok(OrderEventKind::CancelConfirmed),
        "RECONCILE_OPEN" => Ok(OrderEventKind::ReconcileOpen),
        "RECONCILE_MISSING" => Ok(OrderEventKind::ReconcileMissing),
        other => Err(StoreError::InvalidData(format!(
            "unknown order lifecycle event: {other}"
        ))),
    }
}

fn identity(account_id: &str, execution_id: &str, idempotency_key: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}\u{1f}{idempotency_key}")
}

fn attempt_counter_key(account_id: &str, execution_id: &str) -> String {
    format!("{account_id}\u{1f}{execution_id}")
}

#[async_trait]
impl ExecutionStore for InMemoryStore {
    async fn save_normalized_intent(&self, intent: &NormalizedIntent) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .normalized
            .insert(intent.normalized_intent_id.clone(), intent.clone());
        Ok(())
    }

    async fn load_normalized_intent(
        &self,
        normalized_intent_id: &str,
    ) -> Result<NormalizedIntent, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .normalized
            .get(normalized_intent_id)
            .cloned()
            .ok_or_else(|| {
                StoreError::NotFound(format!("normalized_intent_id={normalized_intent_id}"))
            })
    }

    async fn save_snapshot(&self, snapshot: &FeasibilitySnapshot) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .snapshots
            .insert(snapshot.snapshot_id.clone(), snapshot.clone());
        Ok(())
    }

    async fn load_snapshot(&self, snapshot_id: &str) -> Result<FeasibilitySnapshot, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .snapshots
            .get(snapshot_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("snapshot_id={snapshot_id}")))
    }

    async fn save_decision(&self, decision: &ConstraintDecision) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .decisions
            .insert(decision.decision_id.clone(), decision.clone());
        Ok(())
    }

    async fn load_decision(&self, decision_id: &str) -> Result<ConstraintDecision, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .decisions
            .get(decision_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("decision_id={decision_id}")))
    }

    async fn save_plan_summary(&self, plan: &ExecutionPlanSummary) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .plans
            .insert(plan.execution_id.clone(), plan.clone());
        Ok(())
    }

    async fn load_plan_summary(
        &self,
        execution_id: &str,
    ) -> Result<ExecutionPlanSummary, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .plans
            .get(execution_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
    }

    async fn save_order_reservation(
        &self,
        reservation: &OrderReservation,
    ) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .reservations
            .insert(reservation.reservation_id.clone(), reservation.clone());
        Ok(())
    }

    async fn record_submit_receipt(&self, receipt: &SubmitReceipt) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .receipts
            .insert(receipt.execution_id.clone(), receipt.clone());
        Ok(())
    }

    async fn load_submit_receipt(&self, execution_id: &str) -> Result<SubmitReceipt, StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .receipts
            .get(execution_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("execution_id={execution_id}")))
    }
}

#[async_trait]
impl ExecutionLifecycleStore for InMemoryStore {
    async fn record_execution_lifecycle_event(
        &self,
        event: &ExecutionLifecycleEvent,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.lifecycle_event_counter += 1;
        let mut stored = sanitize_execution_lifecycle_event(event.clone());
        stored.event_id = Some(state.lifecycle_event_counter);
        stored.created_at = Some(Utc::now());
        state.lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_execution_lifecycle_events(
        &self,
        query: &ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .lifecycle_events
            .iter()
            .filter(|event| event.execution_id == query.execution_id)
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
impl SignOnlyLifecycleStore for InMemoryStore {
    async fn record_sign_only_lifecycle_event(
        &self,
        record: &SignOnlyLifecycleRecord,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        if !state.plans.contains_key(&record.execution_id.0) {
            return Err(StoreError::NotFound(format!(
                "execution_id={}",
                record.execution_id.0
            )));
        }
        let existing: Vec<_> = state
            .sign_only_lifecycle_events
            .iter()
            .filter(|existing| existing.execution_id == record.execution_id)
            .cloned()
            .collect();
        if sign_only_lifecycle_record_is_replay(&existing, record)? {
            return Ok(());
        }
        validate_sign_only_lifecycle_append_for_store(&existing, record)?;
        state.sign_only_event_counter += 1;
        let mut stored = sanitize_sign_only_lifecycle_record(record.clone());
        stored.event_id = Some(state.sign_only_event_counter);
        stored.created_at = Some(Utc::now());
        state.sign_only_lifecycle_events.push(stored);
        Ok(())
    }

    async fn list_sign_only_lifecycle_events(
        &self,
        query: &SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, StoreError> {
        let mut records: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .sign_only_lifecycle_events
            .iter()
            .filter(|record| record.execution_id.0 == query.execution_id)
            .filter(|record| {
                query
                    .before_event_id
                    .map(|before| record.event_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        records.sort_by_key(|record| record.event_id.unwrap_or(0));
        records.reverse();
        records.truncate(query.bounded_limit());
        records.reverse();
        Ok(records)
    }
}

#[async_trait]
impl RuntimeWorkerObservationStore for InMemoryStore {
    async fn record_runtime_worker_observation(
        &self,
        observation: &RuntimeWorkerObservation,
    ) -> Result<(), StoreError> {
        let mut stored = observation.clone();
        if stored.observed_at.is_none() {
            stored.observed_at = Some(Utc::now());
        }
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_worker_observations
            .push(stored);
        Ok(())
    }
}

#[async_trait]
impl RuntimeWorkerHealthStore for InMemoryStore {
    async fn record_worker_heartbeat(
        &self,
        heartbeat: &RuntimeWorkerHeartbeat,
    ) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .insert(heartbeat.worker_id.clone(), heartbeat.clone());
        Ok(())
    }
}

#[async_trait]
impl RuntimeStateStore for InMemoryStore {
    async fn load_runtime_state(
        &self,
        query: &RuntimeStateQuery,
    ) -> Result<RuntimeStateSummary, StoreError> {
        let mut base = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .runtime_states
            .get(&query.key())
            .cloned()
            .unwrap_or(RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Unknown,
                worker_status: WorkerStatus::Unknown,
                collateral_profile_status: CollateralProfileStatus::Unknown,
                kill_switch_enabled: true,
                required_capabilities: query.required_capabilities.clone(),
            });
        let mut required_capabilities = query.required_capabilities.clone();
        if required_capabilities.is_empty() {
            required_capabilities = base.required_capabilities.clone();
        }
        let heartbeats: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .worker_health
            .values()
            .cloned()
            .collect();
        if !required_capabilities.is_empty() {
            base.worker_status = worker_status_from_heartbeats(&heartbeats, &required_capabilities);
            base.required_capabilities = required_capabilities;
        }
        Ok(apply_runtime_worker_observations(
            base,
            &self.observations_for_account(&query.account_id),
        ))
    }
}

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
impl AdminAuditStore for InMemoryStore {
    async fn record_admin_audit_event(&self, event: &AdminAuditEvent) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        state.admin_audit_counter += 1;
        let mut stored = sanitize_admin_audit_event(event.clone());
        stored.audit_id = Some(state.admin_audit_counter);
        stored.created_at = Some(Utc::now());
        state.admin_audit.push(stored);
        Ok(())
    }

    async fn list_admin_audit_events(
        &self,
        query: &AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, StoreError> {
        let mut events: Vec<_> = self
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .admin_audit
            .iter()
            .filter(|event| {
                query
                    .before_audit_id
                    .map(|before| event.audit_id.unwrap_or(i64::MAX) < before)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .operation
                    .as_ref()
                    .map(|operation| &event.operation == operation)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .principal_subject
                    .as_ref()
                    .map(|principal_subject| &event.principal_subject == principal_subject)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .result
                    .as_ref()
                    .map(|result| &event.result == result)
                    .unwrap_or(true)
            })
            .filter(|event| {
                query
                    .correlation_id
                    .as_ref()
                    .map(|correlation_id| event.correlation_id.as_ref() == Some(correlation_id))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        events.sort_by_key(|event| event.audit_id.unwrap_or(0));
        events.reverse();
        events.truncate(query.bounded_limit());
        events.reverse();
        Ok(events)
    }
}

#[async_trait]
impl IdempotencyStore for InMemoryStore {
    async fn begin_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> Result<IdempotencyAction, StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        if let Some(existing) = state.idempotency.get(&key) {
            if existing.request_fingerprint != request_fingerprint {
                return Ok(IdempotencyAction::Conflict);
            }
            if let (Some(response_fingerprint), Some(response_json)) =
                (&existing.response_fingerprint, &existing.response_json)
            {
                return Ok(IdempotencyAction::ReplayStoredResponse {
                    response_fingerprint: response_fingerprint.clone(),
                    response_json: response_json.clone(),
                });
            }
            return Ok(IdempotencyAction::InProgress {
                submit_attempt: existing.submit_attempt,
                retry_after_ms: 1_000,
            });
        }

        let counter_key = attempt_counter_key(account_id, execution_id);
        let next_attempt = state
            .attempt_counters
            .get(&counter_key)
            .copied()
            .unwrap_or(0)
            + 1;
        state.attempt_counters.insert(counter_key, next_attempt);
        state.idempotency.insert(
            key,
            IdempotencyRecord {
                submit_attempt: next_attempt,
                request_fingerprint: request_fingerprint.into(),
                response_fingerprint: None,
                response_json: None,
            },
        );
        Ok(IdempotencyAction::Proceed {
            submit_attempt: next_attempt,
            owner_token: format!("owner-{account_id}-{execution_id}-{next_attempt}"),
        })
    }

    async fn finish_submit_attempt(
        &self,
        account_id: &str,
        execution_id: &str,
        idempotency_key: &str,
        request_fingerprint: &str,
        response_fingerprint: &str,
        response_json: &str,
    ) -> Result<(), StoreError> {
        let mut state = self.inner.lock().expect("in-memory store mutex poisoned");
        let key = identity(account_id, execution_id, idempotency_key);
        let record = state
            .idempotency
            .get_mut(&key)
            .ok_or_else(|| StoreError::NotFound(key.clone()))?;
        if record.request_fingerprint != request_fingerprint {
            return Err(StoreError::Conflict("request_fingerprint mismatch".into()));
        }
        record.response_fingerprint = Some(response_fingerprint.into());
        record.response_json = Some(response_json.into());
        Ok(())
    }
}

fn runtime_observation_worker_status(
    observations: &[RuntimeWorkerObservation],
) -> Option<WorkerStatus> {
    if observations.is_empty() {
        return None;
    }
    let mut has_degraded = false;
    let mut has_healthy = false;
    for observation in observations {
        let status = observation
            .status
            .trim()
            .to_ascii_uppercase()
            .replace('-', "_");
        if matches!(status.as_str(), "STALE" | "ERROR" | "DOWN") {
            return Some(WorkerStatus::Stale);
        }
        if matches!(status.as_str(), "UNKNOWN" | "UNOBSERVED") {
            return Some(WorkerStatus::Unknown);
        }
        if observation.should_fail_closed || matches!(status.as_str(), "DEGRADED" | "BLOCKED") {
            has_degraded = true;
        }
        if matches!(status.as_str(), "HEALTHY" | "OK" | "ALLOWED") {
            has_healthy = true;
        }
    }
    if has_degraded {
        Some(WorkerStatus::Degraded)
    } else if has_healthy {
        Some(WorkerStatus::Healthy)
    } else {
        Some(WorkerStatus::Unknown)
    }
}

pub fn apply_runtime_worker_observations(
    mut base: RuntimeStateSummary,
    observations: &[RuntimeWorkerObservation],
) -> RuntimeStateSummary {
    if let Some(observed_status) = runtime_observation_worker_status(observations) {
        base.worker_status = worst_worker_status(base.worker_status, observed_status);
        for observation in observations {
            if !base.required_capabilities.contains(&observation.capability) {
                base.required_capabilities
                    .push(observation.capability.clone());
            }
        }
    }
    base
}

fn worst_worker_status(left: WorkerStatus, right: WorkerStatus) -> WorkerStatus {
    use WorkerStatus::*;
    match (left, right) {
        (Stale, _) | (_, Stale) => Stale,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Degraded, _) | (_, Degraded) => Degraded,
        (Healthy, Healthy) => Healthy,
    }
}

fn quantity_bound_to_resource_and_amount(
    bound: &QuantityBound,
) -> Result<(&'static str, &str), StoreError> {
    match bound {
        QuantityBound::WorstCaseQuoteNotional(v) => Ok(("worst_case_quote_notional", &v.0)),
        QuantityBound::WorstCaseBaseShares(v) => Ok(("worst_case_base_shares", &v.0)),
        QuantityBound::Unsupported(reason) => Err(StoreError::Conflict(format!(
            "unsupported quantity bound for reservation: {reason}"
        ))),
    }
}

fn reservation_state_to_str(state: &ReservationState) -> &'static str {
    match state {
        ReservationState::Pending => "PENDING",
        ReservationState::Active => "ACTIVE",
        ReservationState::Released => "RELEASED",
        ReservationState::Consumed => "CONSUMED",
        ReservationState::Orphaned => "ORPHANED",
    }
}

pub fn submit_status_str(status: &SubmitStatus) -> &'static str {
    match status {
        SubmitStatus::Accepted => "ACCEPTED",
        SubmitStatus::Posted => "POSTED",
        SubmitStatus::PartialRemoteUnknown => "PARTIAL_REMOTE_UNKNOWN",
        SubmitStatus::RemoteUnknown => "REMOTE_UNKNOWN",
        SubmitStatus::Rejected => "REJECTED",
        SubmitStatus::Blocked => "BLOCKED",
    }
}

#[cfg(test)]
async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
    store
        .save_plan_summary(&ExecutionPlanSummary {
            execution_id: execution_id.into(),
            account_id: pmx_core::AccountId(account_id.into()),
            normalized_intent_id: format!("norm-{execution_id}"),
            snapshot_id: format!("snap-{execution_id}"),
            decision_id: format!("decision-{execution_id}"),
            plan_hash: pmx_core::HashValue(format!("hash-{execution_id}")),
            status: pmx_core::PlanStatus::Ready,
            max_exposure: pmx_core::DecimalString("0".into()),
            explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
        })
        .await
        .expect("seed execution plan");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_identity_is_documented_in_trait() {
        let action = IdempotencyAction::Proceed {
            submit_attempt: 1,
            owner_token: "owner".into(),
        };
        assert_eq!(
            action,
            IdempotencyAction::Proceed {
                submit_attempt: 1,
                owner_token: "owner".into(),
            }
        );
    }

    #[test]
    fn advisory_lock_key_is_deterministic_and_scoped() {
        let a = advisory_lock_key("submit", "acct-1", "exec-1");
        let b = advisory_lock_key("submit", "acct-1", "exec-1");
        let c = advisory_lock_key("submit", "acct-1", "exec-2");
        let d = advisory_lock_key("reservation", "acct-1", "exec-1");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn maps_plan_status_for_db() {
        let status = submit_status_str(&SubmitStatus::RemoteUnknown);
        assert_eq!(status, "REMOTE_UNKNOWN");
    }

    #[tokio::test]
    async fn in_memory_same_request_without_response_is_in_progress() {
        let store = InMemoryStore::default();
        let first = store
            .begin_submit_attempt("acct", "exec", "idem", "req")
            .await
            .expect("first begin");
        assert!(matches!(first, IdempotencyAction::Proceed { .. }));
        let second = store
            .begin_submit_attempt("acct", "exec", "idem", "req")
            .await
            .expect("second begin");
        assert!(matches!(second, IdempotencyAction::InProgress { .. }));
    }

    #[tokio::test]
    async fn runtime_worker_observations_degrade_loaded_runtime_state() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-runtime-observed",
            "cond-runtime-observed",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: "acct-runtime-observed".into(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "lease expired".into(),
                observed_at: None,
            })
            .await
            .expect("record observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-runtime-observed".into(),
                condition_id: "cond-runtime-observed".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("load runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
        assert!(
            state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    #[tokio::test]
    async fn stale_runtime_worker_observations_are_ignored() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-runtime-stale-observation",
            "cond-runtime-stale-observation",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-runtime-stale-observation".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: "acct-runtime-stale-observation".into(),
                capability: "heartbeat-lease".into(),
                worker_kind: "HeartbeatLease".into(),
                status: "STALE".into(),
                should_fail_closed: true,
                reason: "old lease expiry".into(),
                observed_at: Some(
                    Utc::now() - Duration::seconds(DEFAULT_RUNTIME_OBSERVATION_TTL_SECONDS + 1),
                ),
            })
            .await
            .expect("record stale observation");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-runtime-stale-observation".into(),
                condition_id: "cond-runtime-stale-observation".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("load runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
        assert!(
            !state
                .required_capabilities
                .contains(&"heartbeat-lease".into())
        );
    }

    // Async behavior tests are intentionally split into repository-specific tests.
}

#[cfg(test)]
mod admin_audit_tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_admin_audit_records_without_exposing_secrets() {
        let store = InMemoryStore::default();
        store
            .record_admin_audit_event(&AdminAuditEvent {
                audit_id: None,
                principal_subject: "admin-token".into(),
                operation: "KillSwitch".into(),
                request_fingerprint: Some("abc123".into()),
                correlation_id: Some("corr-admin-test".into()),
                result: "ACCEPTED".into(),
                created_at: None,
            })
            .await
            .expect("record audit event");
        let len = store
            .inner
            .lock()
            .expect("in-memory store mutex poisoned")
            .admin_audit
            .len();
        assert_eq!(len, 1);
    }

    #[tokio::test]
    async fn in_memory_persists_sign_only_lifecycle_records() {
        let store = InMemoryStore::default();
        let execution_id = pmx_core::ExecutionId("exec-sign-only".into());
        let account_id = pmx_core::AccountId("acct-sign-only".into());
        seed_test_plan(&store, &execution_id.0, &account_id.0).await;
        let records_to_append = [
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
                event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::SigningRequested,
                event: pmx_core::SignOnlyLifecycleEventKind::RequestSigning,
                client_event_id: None,
                signed_order_ref: None,
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
            SignOnlyLifecycleRecord {
                execution_id: execution_id.clone(),
                account_id: account_id.clone(),
                state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
                event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
                client_event_id: None,
                signed_order_ref: Some("sign-only:redacted-ref".into()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            },
        ];
        for record in &records_to_append {
            store
                .record_sign_only_lifecycle_event(record)
                .await
                .expect("record sign-only lifecycle");
        }
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.event_id.is_some()));
        assert!(records.iter().all(|record| record.created_at.is_some()));
        assert!(sign_only_lifecycle_records_equivalent(
            records.last().unwrap(),
            records_to_append.last().unwrap()
        ));
    }

    #[tokio::test]
    async fn in_memory_sign_only_replay_is_idempotent() {
        let store = InMemoryStore::default();
        seed_test_plan(&store, "exec-sign-only-replay", "acct-sign-only-replay").await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only-replay".into()),
            account_id: pmx_core::AccountId("acct-sign-only-replay".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: None,
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("record sign-only lifecycle");
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("replay sign-only lifecycle");
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only-replay".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_sign_only_client_event_id_replays_and_rejects_mismatch() {
        let store = InMemoryStore::default();
        seed_test_plan(
            &store,
            "exec-sign-only-client-event",
            "acct-sign-only-client-event",
        )
        .await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only-client-event".into()),
            account_id: pmx_core::AccountId("acct-sign-only-client-event".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: Some("client-event-1".into()),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("record sign-only lifecycle");
        store
            .record_sign_only_lifecycle_event(&record)
            .await
            .expect("replay client_event_id");
        let mut mismatched = record.clone();
        mismatched.event = pmx_core::SignOnlyLifecycleEventKind::Abandon;
        assert!(matches!(
            store.record_sign_only_lifecycle_event(&mismatched).await,
            Err(StoreError::Conflict(_))
        ));
        let records = store
            .list_sign_only_lifecycle_events(&SignOnlyLifecycleQuery {
                execution_id: "exec-sign-only-client-event".into(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].client_event_id.as_deref(),
            Some("client-event-1")
        );
    }

    #[tokio::test]
    async fn in_memory_rejects_sign_only_for_unknown_execution() {
        let store = InMemoryStore::default();
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("missing-exec".into()),
            account_id: pmx_core::AccountId("acct-missing-exec".into()),
            state: pmx_core::SignOnlyLifecycleState::ReservationPrepared,
            event: pmx_core::SignOnlyLifecycleEventKind::PrepareReservation,
            client_event_id: Some("missing-exec-event".into()),
            signed_order_ref: None,
            no_remote_side_effect: true,
            event_id: None,
            created_at: None,
        };
        assert!(matches!(
            store.record_sign_only_lifecycle_event(&record).await,
            Err(StoreError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn in_memory_rejects_sign_only_remote_side_effect_records() {
        let store = InMemoryStore::default();
        seed_test_plan(&store, "exec-sign-only", "acct-sign-only").await;
        let record = SignOnlyLifecycleRecord {
            execution_id: pmx_core::ExecutionId("exec-sign-only".into()),
            account_id: pmx_core::AccountId("acct-sign-only".into()),
            state: pmx_core::SignOnlyLifecycleState::SignedDryRun,
            event: pmx_core::SignOnlyLifecycleEventKind::SignedWithoutPost,
            client_event_id: None,
            signed_order_ref: Some("sign-only:redacted-ref".into()),
            no_remote_side_effect: false,
            event_id: None,
            created_at: None,
        };
        assert!(
            store
                .record_sign_only_lifecycle_event(&record)
                .await
                .is_err()
        );
    }
}

#[cfg(test)]
mod runtime_worker_health_tests_v23 {
    use super::*;
    use pmx_core::{CollateralProfileStatus, GeoblockStatus, RuntimeStateSummary, WorkerStatus};

    #[tokio::test]
    async fn in_memory_worker_heartbeat_informs_runtime_state() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-heartbeat",
            "cond-heartbeat",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Unknown,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-heartbeat-1".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now(),
                last_error: None,
            })
            .await
            .expect("record heartbeat");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-heartbeat".into(),
                condition_id: "cond-heartbeat".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Healthy);
    }

    #[tokio::test]
    async fn stale_in_memory_worker_heartbeat_fails_closed() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test(
            "acct-heartbeat-stale",
            "cond-heartbeat-stale",
            None,
            RuntimeStateSummary {
                geoblock_status: GeoblockStatus::Allowed,
                worker_status: WorkerStatus::Healthy,
                collateral_profile_status: CollateralProfileStatus::DefaultResolved,
                kill_switch_enabled: false,
                required_capabilities: vec!["heartbeat".into()],
            },
        );
        store
            .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                worker_id: "worker-heartbeat-stale".into(),
                role: "Heartbeat".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_heartbeat_at: Utc::now()
                    - Duration::seconds(runtime_observation_ttl_seconds() + 1),
                last_error: Some("missed heartbeat".into()),
            })
            .await
            .expect("record heartbeat");
        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-heartbeat-stale".into(),
                condition_id: "cond-heartbeat-stale".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["heartbeat".into()],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Stale);
    }
}

#[cfg(test)]
mod order_lifecycle_store_tests_v23 {
    use super::*;
    use pmx_core::{OrderEventKind, OrderLifecycleState};

    fn test_order(order_id: &str) -> OrderLifecycleRecord {
        OrderLifecycleRecord {
            order_id: order_id.into(),
            execution_id: format!("exec-{order_id}"),
            account_id: "acct-order-life".into(),
            condition_id: "cond-order-life".into(),
            token_id: "token-order-life".into(),
            side: "BUY".into(),
            lifecycle_state: OrderLifecycleState::Posted,
            remote_order_id: Some(format!("remote-{order_id}")),
            remote_state: Some("OPEN".into()),
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn in_memory_order_lifecycle_records_cancel_requested() {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&test_order("order-life-1"))
            .await
            .expect("upsert order");
        let updated = store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: "order-life-1".into(),
                event: OrderEventKind::CancelRequested,
                event_source: "pmx-store-test".into(),
                payload: serde_json::json!({"no_remote_side_effect": true}),
                created_at: None,
            })
            .await
            .expect("record event");
        assert_eq!(
            updated.lifecycle_state,
            OrderLifecycleState::CancelRequested
        );
        let events = store
            .list_order_lifecycle_events(&OrderLifecycleEventQuery {
                order_id: "order-life-1".into(),
                limit: 10,
                before_event_id: None,
            })
            .await
            .expect("list events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, OrderEventKind::CancelRequested);
        assert!(events[0].event_id.is_some());
    }

    #[tokio::test]
    async fn in_memory_order_lifecycle_rejects_invalid_transition() {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&test_order("order-life-invalid"))
            .await
            .expect("upsert order");
        let err = store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: "order-life-invalid".into(),
                event: OrderEventKind::CancelConfirmed,
                event_source: "pmx-store-test".into(),
                payload: serde_json::json!({}),
                created_at: None,
            })
            .await
            .expect_err("invalid transition");
        assert!(matches!(err, StoreError::Conflict(_)));
    }
}
