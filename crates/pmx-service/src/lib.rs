use async_trait::async_trait;
use chrono::Utc;
use pmx_core::*;
use pmx_policy::evaluate_constraints;
use pmx_runtime::{
    HeartbeatLeaseCandidate, HeartbeatLeaseElection, HeartbeatLeaseElectionInput,
    ResourceRefreshEvaluation, ResourceRefreshEvaluationInput, ResourceRefreshObservation,
    RuntimeSignal, RuntimeWorkerProviderSnapshot, elect_heartbeat_lease_owner,
    evaluate_resource_refresh_freshness, runtime_worker_loop_tick, runtime_worker_store_writes,
};
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore, OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore,
    RuntimeStateQuery, RuntimeStateStore, RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat,
    RuntimeWorkerObservation, RuntimeWorkerObservationStore, SignOnlyLifecycleQuery,
    SignOnlyLifecycleStore, StoreError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_CONTRACT_VERSION: &str = "1.0.0-draft";

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("in progress: retry_after_ms={retry_after_ms}")]
    InProgress { retry_after_ms: u64 },
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub normalized_intent: NormalizedIntent,
    pub snapshot: FeasibilitySnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionByIdRequest {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanCommand {
    pub normalized_intent: NormalizedIntent,
    pub snapshot: FeasibilitySnapshot,
    pub decision: ConstraintDecision,
    pub approval: ApprovalReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePlanByIdCommand {
    pub normalized_intent_id: String,
    pub snapshot_id: String,
    pub decision_id: String,
    pub approval: ApprovalReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlanCommand {
    pub execution_id: String,
    pub plan_hash: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    Accepted(SubmitReceipt),
    Replayed(SubmitReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTick {
    pub worker_id: String,
    pub role: String,
    pub capability: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default)]
    pub signals: Vec<RuntimeSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerTickReceipt {
    pub worker_id: String,
    pub capability: String,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerProviderTickReceipt {
    pub worker_id: String,
    pub provider_name: String,
    pub lease_owner_active: bool,
    pub submit_allowed_by_runtime: bool,
    pub heartbeat_recorded: bool,
    pub observations_recorded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatLeaseElectionTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub candidates: Vec<HeartbeatLeaseCandidate>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatLeaseElectionTickReceipt {
    pub election: HeartbeatLeaseElection,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRefreshWorkerTick {
    pub account_id: String,
    pub provider_name: String,
    pub instance_id: String,
    pub lease_owner_id: String,
    pub market_websocket_connected: bool,
    pub market_websocket_stale: bool,
    pub user_websocket_connected: bool,
    pub user_websocket_stale: bool,
    pub geoblock_status: GeoblockStatus,
    pub remote_unknown_orders: u32,
    pub observations: Vec<ResourceRefreshObservation>,
    pub observed_at: chrono::DateTime<Utc>,
    pub stale_after_seconds: i64,
    pub no_trading_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRefreshWorkerTickReceipt {
    pub evaluation: ResourceRefreshEvaluation,
    pub provider_tick: RuntimeWorkerProviderTickReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionRequest {
    pub execution_id: String,
    pub account_id: String,
    pub plan_hash: String,
    pub signed_order_ref: String,
    pub no_remote_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardSignOnlyConstructionReceipt {
    pub execution_id: String,
    pub signed_order_ref: String,
    pub lifecycle_records: Vec<SignOnlyLifecycleRecord>,
    pub no_remote_side_effect: bool,
}

#[async_trait]
pub trait RuntimeStateProvider: Clone + Send + Sync + 'static {
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary;
}

pub async fn record_runtime_worker_signals<S>(
    store: &S,
    account_id: impl Into<String>,
    signals: &[RuntimeSignal],
) -> Result<usize, ServiceError>
where
    S: RuntimeWorkerObservationStore + Send + Sync,
{
    let writes = runtime_worker_store_writes(account_id, signals);
    for write in &writes {
        store
            .record_runtime_worker_observation(&RuntimeWorkerObservation {
                account_id: write.account_id.clone(),
                capability: write.capability.clone(),
                worker_kind: format!("{:?}", write.worker_kind),
                status: format!("{:?}", write.status),
                should_fail_closed: write.should_fail_closed,
                reason: write.reason.clone(),
                observed_at: None,
            })
            .await?;
    }
    Ok(writes.len())
}

pub async fn record_runtime_worker_tick<S>(
    store: &S,
    account_id: impl Into<String>,
    tick: RuntimeWorkerTick,
) -> Result<RuntimeWorkerTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.worker_id.trim().is_empty()
        || tick.role.trim().is_empty()
        || tick.capability.trim().is_empty()
        || tick.status.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "worker_id, role, capability and status must be non-empty".into(),
        ));
    }
    store
        .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
            worker_id: tick.worker_id.clone(),
            role: tick.role.clone(),
            capability: tick.capability.clone(),
            status: tick.status.clone(),
            last_heartbeat_at: Utc::now(),
            last_error: tick.last_error.clone(),
        })
        .await?;
    let observations_recorded =
        record_runtime_worker_signals(store, account_id, &tick.signals).await?;
    Ok(RuntimeWorkerTickReceipt {
        worker_id: tick.worker_id,
        capability: tick.capability,
        heartbeat_recorded: true,
        observations_recorded,
    })
}

pub async fn record_runtime_worker_provider_snapshot<S>(
    store: &S,
    snapshot: RuntimeWorkerProviderSnapshot,
) -> Result<RuntimeWorkerProviderTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if snapshot.provider_name.trim().is_empty()
        || snapshot.instance_id.trim().is_empty()
        || snapshot.account_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "provider_name, instance_id and account_id must be non-empty".into(),
        ));
    }
    if !snapshot.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "runtime worker provider snapshots must not contain trading side effects".into(),
        ));
    }
    let account_id = snapshot.account_id.clone();
    let provider_name = snapshot.provider_name.clone();
    let instance_id = snapshot.instance_id.clone();
    let tick = runtime_worker_loop_tick(snapshot.into_loop_input());
    let status = if tick.submit_allowed_by_runtime {
        "HEALTHY"
    } else {
        "DEGRADED"
    };
    let receipt = record_runtime_worker_tick(
        store,
        account_id,
        RuntimeWorkerTick {
            worker_id: instance_id.clone(),
            role: provider_name.clone(),
            capability: "runtime-worker-loop".into(),
            status: status.into(),
            last_error: (!tick.submit_allowed_by_runtime)
                .then(|| "runtime worker loop fail-closed".into()),
            signals: tick.signals,
        },
    )
    .await?;
    Ok(RuntimeWorkerProviderTickReceipt {
        worker_id: instance_id,
        provider_name,
        lease_owner_active: tick.lease_owner_active,
        submit_allowed_by_runtime: tick.submit_allowed_by_runtime,
        heartbeat_recorded: receipt.heartbeat_recorded,
        observations_recorded: receipt.observations_recorded,
    })
}

pub async fn record_heartbeat_lease_election_tick<S>(
    store: &S,
    tick: HeartbeatLeaseElectionTick,
) -> Result<HeartbeatLeaseElectionTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.account_id.trim().is_empty()
        || tick.provider_name.trim().is_empty()
        || tick.instance_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, provider_name and instance_id must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "heartbeat lease election ticks must not contain trading side effects".into(),
        ));
    }
    let election = elect_heartbeat_lease_owner(HeartbeatLeaseElectionInput {
        instance_id: tick.instance_id.clone(),
        candidates: tick.candidates,
        observed_at: tick.observed_at,
        stale_after_seconds: tick.stale_after_seconds,
    });
    let provider_tick = record_runtime_worker_provider_snapshot(
        store,
        RuntimeWorkerProviderSnapshot {
            account_id: tick.account_id,
            lease_owner_id: election.lease_owner_id.clone(),
            instance_id: tick.instance_id,
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(HeartbeatLeaseElectionTickReceipt {
        election,
        provider_tick,
    })
}

pub async fn record_resource_refresh_worker_tick<S>(
    store: &S,
    tick: ResourceRefreshWorkerTick,
) -> Result<ResourceRefreshWorkerTickReceipt, ServiceError>
where
    S: RuntimeWorkerHealthStore + RuntimeWorkerObservationStore + Send + Sync,
{
    if tick.account_id.trim().is_empty()
        || tick.provider_name.trim().is_empty()
        || tick.instance_id.trim().is_empty()
    {
        return Err(ServiceError::BadRequest(
            "account_id, provider_name and instance_id must be non-empty".into(),
        ));
    }
    if !tick.no_trading_side_effect {
        return Err(ServiceError::BadRequest(
            "resource refresh worker ticks must not contain trading side effects".into(),
        ));
    }
    let evaluation = evaluate_resource_refresh_freshness(ResourceRefreshEvaluationInput {
        observations: tick.observations,
        observed_at: tick.observed_at,
        stale_after_seconds: tick.stale_after_seconds,
    });
    let provider_tick = record_runtime_worker_provider_snapshot(
        store,
        RuntimeWorkerProviderSnapshot {
            account_id: tick.account_id,
            lease_owner_id: tick.lease_owner_id,
            instance_id: tick.instance_id,
            market_websocket_connected: tick.market_websocket_connected,
            market_websocket_stale: tick.market_websocket_stale,
            user_websocket_connected: tick.user_websocket_connected,
            user_websocket_stale: tick.user_websocket_stale,
            geoblock_status: tick.geoblock_status,
            resource_refresh_fresh: evaluation.fresh,
            remote_unknown_orders: tick.remote_unknown_orders,
            observed_at: tick.observed_at,
            provider_name: tick.provider_name,
            no_trading_side_effect: true,
        },
    )
    .await?;
    Ok(ResourceRefreshWorkerTickReceipt {
        evaluation,
        provider_tick,
    })
}

fn fail_closed_runtime_state(required_capabilities: Vec<String>) -> RuntimeStateSummary {
    RuntimeStateSummary {
        geoblock_status: GeoblockStatus::Unknown,
        worker_status: WorkerStatus::Unknown,
        collateral_profile_status: CollateralProfileStatus::Unknown,
        kill_switch_enabled: true,
        required_capabilities,
    }
}

#[derive(Debug, Clone, Default)]
pub struct FailClosedRuntimeStateProvider;

#[async_trait]
impl RuntimeStateProvider for FailClosedRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        fail_closed_runtime_state(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct StaticRuntimeStateProvider {
    runtime_state: RuntimeStateSummary,
}

impl StaticRuntimeStateProvider {
    pub fn new(runtime_state: RuntimeStateSummary) -> Self {
        Self { runtime_state }
    }
}

#[async_trait]
impl RuntimeStateProvider for StaticRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        self.runtime_state.clone()
    }
}

#[derive(Debug, Clone)]
pub struct StoreBackedRuntimeStateProvider<S> {
    store: S,
    required_capabilities: Vec<String>,
}

impl<S> StoreBackedRuntimeStateProvider<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            required_capabilities: vec![
                "heartbeat".into(),
                "reconcile".into(),
                "resource-refresh".into(),
            ],
        }
    }

    pub fn with_required_capabilities(store: S, required_capabilities: Vec<String>) -> Self {
        Self {
            store,
            required_capabilities,
        }
    }
}

#[async_trait]
impl<S> RuntimeStateProvider for StoreBackedRuntimeStateProvider<S>
where
    S: RuntimeStateStore + Clone + Send + Sync + 'static,
{
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        let query = RuntimeStateQuery {
            account_id: normalized_intent.account_id.0.clone(),
            condition_id: normalized_intent.market.condition_id.0.clone(),
            collateral_profile_id: normalized_intent.collateral_profile_id.clone(),
            required_capabilities: self.required_capabilities.clone(),
        };
        self.store
            .load_runtime_state(&query)
            .await
            .unwrap_or_else(|_| fail_closed_runtime_state(query.required_capabilities))
    }
}

#[derive(Debug, Clone)]
pub struct ExecutorService<S, R = FailClosedRuntimeStateProvider> {
    store: S,
    runtime_state_provider: R,
    executor_version: String,
    contract_version: String,
}

impl<S> ExecutorService<S, FailClosedRuntimeStateProvider>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + SignOnlyLifecycleStore
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(store: S) -> Self {
        Self::with_runtime_provider(
            store,
            FailClosedRuntimeStateProvider,
            env!("CARGO_PKG_VERSION").to_owned(),
            DEFAULT_CONTRACT_VERSION.to_owned(),
        )
    }
}

impl<S, R> ExecutorService<S, R>
where
    S: ExecutionStore
        + IdempotencyStore
        + AdminAuditStore
        + ExecutionLifecycleStore
        + OrderLifecycleStore
        + SignOnlyLifecycleStore
        + Clone
        + Send
        + Sync
        + 'static,
    R: RuntimeStateProvider,
{
    pub fn with_runtime_provider(
        store: S,
        runtime_state_provider: R,
        executor_version: String,
        contract_version: String,
    ) -> Self {
        Self {
            store,
            runtime_state_provider,
            executor_version,
            contract_version,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn record_admin_audit_event(
        &self,
        event: AdminAuditEvent,
    ) -> Result<(), ServiceError> {
        self.store.record_admin_audit_event(&event).await?;
        Ok(())
    }

    pub async fn list_admin_audit_events(
        &self,
        query: AdminAuditQuery,
    ) -> Result<Vec<AdminAuditEvent>, ServiceError> {
        Ok(self.store.list_admin_audit_events(&query).await?)
    }

    pub async fn record_execution_lifecycle_event(
        &self,
        event: ExecutionLifecycleEvent,
    ) -> Result<(), ServiceError> {
        self.store.record_execution_lifecycle_event(&event).await?;
        Ok(())
    }

    pub async fn list_execution_lifecycle_events(
        &self,
        query: ExecutionLifecycleQuery,
    ) -> Result<Vec<ExecutionLifecycleEvent>, ServiceError> {
        Ok(self.store.list_execution_lifecycle_events(&query).await?)
    }

    pub async fn record_non_live_cancel_request(
        &self,
        order_id: &str,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        if order_id.trim().is_empty() || reason.trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "order_id and reason must be non-empty".into(),
            ));
        }
        if self.store.load_order_lifecycle(order_id).await?.is_none() {
            return Ok(None);
        }
        let updated = self
            .store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: order_id.to_owned(),
                event: OrderEventKind::CancelRequested,
                event_source: "pmx-service".into(),
                payload: serde_json::json!({
                    "kind": "cancel_requested_non_live",
                    "correlation_id": correlation_id,
                    "reason_len": reason.len(),
                    "no_remote_side_effect": true,
                }),
                created_at: None,
            })
            .await?;
        Ok(Some(updated))
    }

    pub async fn record_non_live_reconcile_observation(
        &self,
        order_id: &str,
        event: OrderEventKind,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        if order_id.trim().is_empty() || reason.trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "order_id and reason must be non-empty".into(),
            ));
        }
        if !matches!(
            event,
            OrderEventKind::ReconcileOpen | OrderEventKind::ReconcileMissing
        ) {
            return Err(ServiceError::BadRequest(
                "reconcile observation must be ReconcileOpen or ReconcileMissing".into(),
            ));
        }
        if self.store.load_order_lifecycle(order_id).await?.is_none() {
            return Ok(None);
        }
        let updated = self
            .store
            .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                event_id: None,
                order_id: order_id.to_owned(),
                event,
                event_source: "pmx-service".into(),
                payload: serde_json::json!({
                    "kind": "reconcile_observed_non_live",
                    "correlation_id": correlation_id,
                    "reason_len": reason.len(),
                    "no_remote_side_effect": true,
                }),
                created_at: None,
            })
            .await?;
        Ok(Some(updated))
    }

    pub async fn reconcile_order_lifecycle_divergence(
        &self,
        order_id: &str,
        account_id: Option<&str>,
        remote_observation: RemoteOrderObservation,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<(OrderLifecycleDivergence, Option<OrderLifecycleRecord>)>, ServiceError>
    {
        if order_id.trim().is_empty() || reason.trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "order_id and reason must be non-empty".into(),
            ));
        }
        let Some(order) = self.store.load_order_lifecycle(order_id).await? else {
            return Ok(None);
        };
        if let Some(account_id) = account_id
            && order.account_id != account_id
        {
            return Err(ServiceError::Conflict(
                "order lifecycle account_id does not match request".into(),
            ));
        }
        let divergence =
            classify_order_lifecycle_divergence(&order.lifecycle_state, remote_observation);
        let updated = if let Some(event) = divergence.event.clone() {
            Some(
                self.store
                    .record_order_lifecycle_event(&OrderLifecycleEventRecord {
                        event_id: None,
                        order_id: order_id.to_owned(),
                        event,
                        event_source: "pmx-service".into(),
                        payload: serde_json::json!({
                            "kind": "order_lifecycle_divergence_non_live",
                            "correlation_id": correlation_id,
                            "operator_required": divergence.operator_required,
                            "reason_len": reason.len(),
                            "classification": format!("{:?}", divergence.kind),
                            "no_remote_side_effect": true,
                        }),
                        created_at: None,
                    })
                    .await?,
            )
        } else {
            None
        };
        Ok(Some((divergence, updated)))
    }

    pub async fn record_sign_only_lifecycle_event(
        &self,
        mut record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        record.event_id = None;
        record.created_at = None;
        let query = SignOnlyLifecycleQuery {
            execution_id: record.execution_id.0.clone(),
            limit: 500,
            before_event_id: None,
        };
        let existing = self.store.list_sign_only_lifecycle_events(&query).await?;
        validate_sign_only_lifecycle_append(&existing, &record)?;
        self.store.record_sign_only_lifecycle_event(&record).await?;
        let updated = self.store.list_sign_only_lifecycle_events(&query).await?;
        let matched = if let Some(client_event_id) = record.client_event_id.as_deref() {
            updated
                .iter()
                .rev()
                .find(|candidate| candidate.client_event_id.as_deref() == Some(client_event_id))
        } else {
            updated
                .iter()
                .rev()
                .find(|candidate| sign_only_lifecycle_records_equivalent(candidate, &record))
        };
        Ok(matched.cloned().unwrap_or(record))
    }

    pub async fn list_sign_only_lifecycle_events(
        &self,
        query: SignOnlyLifecycleQuery,
    ) -> Result<Vec<SignOnlyLifecycleRecord>, ServiceError> {
        Ok(self.store.list_sign_only_lifecycle_events(&query).await?)
    }

    pub async fn record_standard_sign_only_construction(
        &self,
        req: StandardSignOnlyConstructionRequest,
    ) -> Result<StandardSignOnlyConstructionReceipt, ServiceError> {
        if req.execution_id.trim().is_empty()
            || req.account_id.trim().is_empty()
            || req.plan_hash.trim().is_empty()
            || req.signed_order_ref.trim().is_empty()
        {
            return Err(ServiceError::BadRequest(
                "execution_id, account_id, plan_hash and signed_order_ref must be non-empty".into(),
            ));
        }
        if !req.no_remote_side_effect {
            return Err(ServiceError::BadRequest(
                "standard sign-only construction must not contain remote side effects".into(),
            ));
        }
        if !req.signed_order_ref.starts_with("sign-only:") {
            return Err(ServiceError::BadRequest(
                "standard sign-only construction requires a redacted sign-only ref".into(),
            ));
        }
        let plan = self.store.load_plan_summary(&req.execution_id).await?;
        if plan.account_id.0 != req.account_id {
            return Err(ServiceError::Conflict(
                "sign-only construction account_id does not match execution plan".into(),
            ));
        }
        if plan.plan_hash.0 != req.plan_hash {
            return Err(ServiceError::Conflict(
                "sign-only construction plan_hash does not match execution plan".into(),
            ));
        }

        let stages = [
            (
                SignOnlyLifecycleEventKind::PrepareReservation,
                SignOnlyLifecycleState::ReservationPrepared,
                None,
                "prepare-reservation",
            ),
            (
                SignOnlyLifecycleEventKind::RequestSigning,
                SignOnlyLifecycleState::SigningRequested,
                None,
                "request-signing",
            ),
            (
                SignOnlyLifecycleEventKind::SignedWithoutPost,
                SignOnlyLifecycleState::SignedDryRun,
                Some(req.signed_order_ref.clone()),
                "signed-without-post",
            ),
        ];
        let mut lifecycle_records = Vec::with_capacity(stages.len());
        for (event, state, signed_order_ref, stage) in stages {
            let record = self
                .record_sign_only_lifecycle_event(SignOnlyLifecycleRecord {
                    execution_id: ExecutionId(req.execution_id.clone()),
                    account_id: AccountId(req.account_id.clone()),
                    state,
                    event,
                    client_event_id: Some(format!("sdk-standard:{}:{stage}", req.plan_hash)),
                    signed_order_ref,
                    no_remote_side_effect: true,
                    event_id: None,
                    created_at: None,
                })
                .await?;
            lifecycle_records.push(record);
        }

        Ok(StandardSignOnlyConstructionReceipt {
            execution_id: req.execution_id,
            signed_order_ref: req.signed_order_ref,
            lifecycle_records,
            no_remote_side_effect: true,
        })
    }

    pub async fn normalize(&self, intent: TradeIntent) -> Result<NormalizedIntent, ServiceError> {
        let normalized =
            normalize_intent(intent).map_err(|err| ServiceError::BadRequest(err.to_string()))?;
        self.store.save_normalized_intent(&normalized).await?;
        Ok(normalized)
    }

    pub async fn capture_snapshot(
        &self,
        normalized: NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        self.store.save_normalized_intent(&normalized).await?;
        let snapshot = self.build_snapshot(&normalized).await?;
        self.store.save_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    pub async fn evaluate_decision(
        &self,
        req: DecisionRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
        self.store
            .save_normalized_intent(&req.normalized_intent)
            .await?;
        self.store.save_snapshot(&req.snapshot).await?;
        let decision = evaluate_constraints(&req.normalized_intent, &req.snapshot);
        self.store.save_decision(&decision).await?;
        Ok(decision)
    }

    /// Evaluate constraints by loading the object graph from the executor store.
    ///
    /// This is the preferred public API path from v0.14 onward: the control plane supplies
    /// only server-issued IDs, and the executor validates object ownership before computing
    /// the decision. Full-object methods remain available for internal tests and migration-free
    /// development but must not be used for live funds paths.
    pub async fn evaluate_decision_by_id(
        &self,
        req: DecisionByIdRequest,
    ) -> Result<ConstraintDecision, ServiceError> {
        let normalized = self
            .store
            .load_normalized_intent(&req.normalized_intent_id)
            .await?;
        let snapshot = self.store.load_snapshot(&req.snapshot_id).await?;
        self.evaluate_decision(DecisionRequest {
            normalized_intent: normalized,
            snapshot,
        })
        .await
    }

    pub async fn compile_plan(
        &self,
        req: CompilePlanCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
        verify_decision_binding(&req.normalized_intent, &req.snapshot, &req.decision)?;
        self.store
            .save_normalized_intent(&req.normalized_intent)
            .await?;
        self.store.save_snapshot(&req.snapshot).await?;
        self.store.save_decision(&req.decision).await?;
        self.build_and_save_plan(
            &req.normalized_intent,
            &req.snapshot,
            &req.decision,
            &req.approval,
        )
        .await
    }

    /// Compile a plan by loading all prior objects from the executor store.
    ///
    /// This prevents client-side object graph splicing such as Intent A + Snapshot B + Decision C.
    pub async fn compile_plan_by_id(
        &self,
        req: CompilePlanByIdCommand,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        let normalized = self
            .store
            .load_normalized_intent(&req.normalized_intent_id)
            .await?;
        let snapshot = self.store.load_snapshot(&req.snapshot_id).await?;
        let decision = self.store.load_decision(&req.decision_id).await?;
        verify_snapshot_binding(&normalized, &snapshot)?;
        verify_decision_binding(&normalized, &snapshot, &decision)?;
        self.build_and_save_plan(&normalized, &snapshot, &decision, &req.approval)
            .await
    }

    async fn build_and_save_plan(
        &self,
        normalized: &NormalizedIntent,
        snapshot: &FeasibilitySnapshot,
        decision: &ConstraintDecision,
        approval: &ApprovalReceipt,
    ) -> Result<ExecutionPlanSummary, ServiceError> {
        let status = if matches!(decision.status, DecisionStatus::Allow) {
            PlanStatus::Ready
        } else {
            PlanStatus::Blocked
        };
        let execution_id = format!("exec-{}", normalized.normalized_intent_id);
        let mut plan = ExecutionPlanSummary {
            execution_id,
            account_id: normalized.account_id.clone(),
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            plan_hash: HashValue("pending".into()),
            status,
            max_exposure: DecimalString("0".into()),
            explanation: vec![
                "v0.15 server-authoritative ID-bound service with admin audit scaffold; live signing/posting remain disabled".into(),
                format!("approval_id={}", approval.approval_id),
                format!("snapshot_id={}", snapshot.snapshot_id),
            ],
        };
        plan.plan_hash = canonical_json_sha256(&PlanHashInput::from(&plan))
            .map_err(|err| ServiceError::Internal(err.to_string()))?;
        self.store.save_plan_summary(&plan).await?;
        Ok(plan)
    }

    pub async fn submit_plan(&self, req: SubmitPlanCommand) -> Result<SubmitOutcome, ServiceError> {
        let plan = self.store.load_plan_summary(&req.execution_id).await?;
        if plan.plan_hash.0 != req.plan_hash {
            return Err(ServiceError::Conflict(
                "plan_hash does not match server-authoritative plan".into(),
            ));
        }
        if !matches!(plan.status, PlanStatus::Ready | PlanStatus::Blocked) {
            return Err(ServiceError::Conflict("plan status is invalid".into()));
        }
        let request_fingerprint = canonical_json_sha256(&req)
            .map_err(|err| ServiceError::Internal(err.to_string()))?
            .0;
        match self
            .store
            .begin_submit_attempt(
                &plan.account_id.0,
                &plan.execution_id,
                &req.idempotency_key,
                &request_fingerprint,
            )
            .await?
        {
            IdempotencyAction::ReplayStoredResponse { response_json, .. } => {
                let receipt: SubmitReceipt =
                    serde_json::from_str(&response_json).map_err(|err| {
                        ServiceError::Internal(format!("stored submit receipt is invalid: {err}"))
                    })?;
                Ok(SubmitOutcome::Replayed(receipt))
            }
            IdempotencyAction::Conflict => Err(ServiceError::Conflict(
                "idempotency key reused with different request fingerprint".into(),
            )),
            IdempotencyAction::InProgress { retry_after_ms, .. } => {
                Err(ServiceError::InProgress { retry_after_ms })
            }
            IdempotencyAction::Proceed { submit_attempt, .. } => {
                if matches!(plan.status, PlanStatus::Ready) {
                    let reservation = OrderReservation {
                        reservation_id: format!("res-{}-{submit_attempt}", plan.execution_id),
                        account_id: plan.account_id.clone(),
                        execution_id: ExecutionId(plan.execution_id.clone()),
                        internal_order_id: None,
                        quantity_bound: QuantityBound::WorstCaseQuoteNotional(DecimalString(
                            "0.00000001".into(),
                        )),
                        state: ReservationState::Pending,
                    };
                    self.store.save_order_reservation(&reservation).await?;
                }
                let receipt = SubmitReceipt {
                    execution_id: req.execution_id,
                    receipt_id: format!("receipt-blocked-{submit_attempt}-{}", Uuid::new_v4()),
                    status: SubmitStatus::Blocked,
                    executor_version: self.executor_version.clone(),
                    contract_version: self.contract_version.clone(),
                };
                let response_json = serde_json::to_string(&receipt).map_err(|err| {
                    ServiceError::Internal(format!("submit receipt serialization failed: {err}"))
                })?;
                let response_fingerprint = canonical_json_sha256(&receipt)
                    .map_err(|err| ServiceError::Internal(err.to_string()))?
                    .0;
                self.store
                    .record_execution_lifecycle_event(&ExecutionLifecycleEvent {
                        event_id: None,
                        execution_id: plan.execution_id.clone(),
                        account_id: plan.account_id.0.clone(),
                        event_type: "SUBMIT_BLOCKED_BEFORE_REMOTE".into(),
                        event_source: "pmx-service".into(),
                        payload: serde_json::json!({
                            "submit_attempt": submit_attempt,
                            "plan_status": format!("{:?}", plan.status),
                            "no_remote_side_effect": true,
                            "receipt_id": receipt.receipt_id.clone(),
                        }),
                        created_at: None,
                    })
                    .await?;
                self.store.record_submit_receipt(&receipt).await?;
                self.store
                    .finish_submit_attempt(
                        &plan.account_id.0,
                        &plan.execution_id,
                        &req.idempotency_key,
                        &request_fingerprint,
                        &response_fingerprint,
                        &response_json,
                    )
                    .await?;
                Ok(SubmitOutcome::Accepted(receipt))
            }
        }
    }

    pub async fn load_submit_receipt(
        &self,
        execution_id: &str,
    ) -> Result<SubmitReceipt, ServiceError> {
        Ok(self.store.load_submit_receipt(execution_id).await?)
    }

    async fn build_snapshot(
        &self,
        normalized: &NormalizedIntent,
    ) -> Result<FeasibilitySnapshot, ServiceError> {
        let snapshot_id = Uuid::new_v4().to_string();
        let runtime_state = self
            .runtime_state_provider
            .capture_runtime_state(normalized)
            .await;
        let captured_at = Utc::now();
        let hash_input = SnapshotHashInput {
            snapshot_id: &snapshot_id,
            normalized_intent_id: &normalized.normalized_intent_id,
            runtime_state: &runtime_state,
            captured_at,
        };
        let snapshot_hash = canonical_json_sha256(&hash_input)
            .map_err(|err| ServiceError::Internal(err.to_string()))?;
        Ok(FeasibilitySnapshot {
            snapshot_id,
            snapshot_hash,
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            runtime_state,
            captured_at,
        })
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotHashInput<'a> {
    snapshot_id: &'a str,
    normalized_intent_id: &'a str,
    runtime_state: &'a RuntimeStateSummary,
    captured_at: chrono::DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct PlanHashInput<'a> {
    execution_id: &'a str,
    account_id: &'a AccountId,
    normalized_intent_id: &'a str,
    snapshot_id: &'a str,
    decision_id: &'a str,
    status: &'a PlanStatus,
    max_exposure: &'a DecimalString,
}

impl<'a> From<&'a ExecutionPlanSummary> for PlanHashInput<'a> {
    fn from(plan: &'a ExecutionPlanSummary) -> Self {
        Self {
            execution_id: &plan.execution_id,
            account_id: &plan.account_id,
            normalized_intent_id: &plan.normalized_intent_id,
            snapshot_id: &plan.snapshot_id,
            decision_id: &plan.decision_id,
            status: &plan.status,
            max_exposure: &plan.max_exposure,
        }
    }
}

fn validate_sign_only_lifecycle_append(
    existing: &[SignOnlyLifecycleRecord],
    record: &SignOnlyLifecycleRecord,
) -> Result<(), ServiceError> {
    if !record.no_remote_side_effect {
        return Err(ServiceError::BadRequest(
            "sign-only lifecycle record must not contain remote side effects".into(),
        ));
    }
    if existing
        .last()
        .map(|last| sign_only_lifecycle_records_equivalent(last, record))
        .unwrap_or(false)
    {
        return Ok(());
    }
    if let Some(first) = existing.first()
        && first.account_id != record.account_id
    {
        return Err(ServiceError::Conflict(
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
        return Err(ServiceError::Conflict(
            "sign-only lifecycle is already terminal".into(),
        ));
    }
    let expected = transition_sign_only_lifecycle(from.clone(), record.event.clone())
        .map_err(|err| ServiceError::Conflict(err.to_string()))?;
    if expected != record.state {
        return Err(ServiceError::Conflict(format!(
            "sign-only lifecycle state mismatch: event {:?} from {:?} yields {:?}, got {:?}",
            record.event, from, expected, record.state
        )));
    }
    match (&record.state, record.signed_order_ref.as_ref()) {
        (SignOnlyLifecycleState::SignedDryRun, Some(value)) if !value.trim().is_empty() => {}
        (SignOnlyLifecycleState::SignedDryRun, _) => {
            return Err(ServiceError::BadRequest(
                "SignedDryRun sign-only lifecycle record requires a non-empty signed_order_ref"
                    .into(),
            ));
        }
        (_, Some(_)) => {
            return Err(ServiceError::BadRequest(
                "signed_order_ref is only allowed for SignedDryRun sign-only lifecycle records"
                    .into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

pub fn verify_snapshot_binding(
    normalized_intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
) -> Result<(), ServiceError> {
    if snapshot.normalized_intent_id != normalized_intent.normalized_intent_id {
        return Err(ServiceError::Conflict(
            "snapshot does not belong to normalized intent".into(),
        ));
    }
    Ok(())
}

pub fn verify_decision_binding(
    normalized_intent: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
) -> Result<(), ServiceError> {
    let expected = evaluate_constraints(normalized_intent, snapshot);
    if &expected != decision {
        return Err(ServiceError::Conflict(
            "decision does not match server recomputation for normalized intent and snapshot"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_store::{
        InMemoryStore, OrderLifecycleStore, RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat,
    };

    fn intent() -> TradeIntent {
        TradeIntent {
            client_intent_id: "client-1".into(),
            account_id: AccountId("acct-1".into()),
            market: MarketRef {
                condition_id: ConditionId("cond-1".into()),
                slug: Some("slug".into()),
                is_sports: false,
            },
            token_id: TokenId("token-1".into()),
            side: Side::Buy,
            quantity: QuantityIntent {
                max_notional: Some(DecimalString("1".into())),
                max_shares: None,
            },
            limit_price: DecimalString("0.5".into()),
            time_in_force: TimeInForce::Gtc,
            collateral_profile_id: None,
        }
    }

    fn allow_runtime_state() -> RuntimeStateSummary {
        RuntimeStateSummary {
            geoblock_status: GeoblockStatus::Allowed,
            worker_status: WorkerStatus::Healthy,
            collateral_profile_status: CollateralProfileStatus::DefaultResolved,
            kill_switch_enabled: false,
            required_capabilities: vec![],
        }
    }

    fn approval() -> ApprovalReceipt {
        ApprovalReceipt {
            approval_id: "approval-1".into(),
            approved_by: "operator".into(),
            approved_at: Utc::now(),
            approval_hash: HashValue("approval-hash".into()),
        }
    }

    fn order(order_id: &str, lifecycle_state: OrderLifecycleState) -> OrderLifecycleRecord {
        OrderLifecycleRecord {
            order_id: order_id.into(),
            execution_id: "exec-order-life".into(),
            account_id: "acct-1".into(),
            condition_id: "cond-1".into(),
            token_id: "token-1".into(),
            side: "BUY".into(),
            lifecycle_state,
            remote_order_id: Some(format!("remote-{order_id}")),
            remote_state: Some("OPEN".into()),
            created_at: None,
            updated_at: None,
        }
    }

    async fn seed_test_plan(store: &InMemoryStore, execution_id: &str, account_id: &str) {
        store
            .save_plan_summary(&ExecutionPlanSummary {
                execution_id: execution_id.into(),
                account_id: AccountId(account_id.into()),
                normalized_intent_id: format!("norm-{execution_id}"),
                snapshot_id: format!("snap-{execution_id}"),
                decision_id: format!("decision-{execution_id}"),
                plan_hash: HashValue(format!("hash-{execution_id}")),
                status: PlanStatus::Ready,
                max_exposure: DecimalString("0".into()),
                explanation: vec!["test plan for sign-only lifecycle FK parity".into()],
            })
            .await
            .expect("seed execution plan");
    }

    #[tokio::test]
    async fn service_flow_persists_and_blocks_submit() {
        let service = ExecutorService::new(InMemoryStore::default());
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        let decision = service
            .evaluate_decision(DecisionRequest {
                normalized_intent: normalized.clone(),
                snapshot: snapshot.clone(),
            })
            .await
            .expect("decision");
        let plan = service
            .compile_plan(CompilePlanCommand {
                normalized_intent: normalized,
                snapshot,
                decision,
                approval: approval(),
            })
            .await
            .expect("plan");
        let outcome = service
            .submit_plan(SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-1".into(),
            })
            .await
            .expect("submit");
        match outcome {
            SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
            SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
        }
    }

    #[tokio::test]
    async fn service_id_bound_flow_persists_and_blocks_submit() {
        let service = ExecutorService::new(InMemoryStore::default());
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision by id");
        let plan = service
            .compile_plan_by_id(CompilePlanByIdCommand {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
                decision_id: decision.decision_id.clone(),
                approval: approval(),
            })
            .await
            .expect("plan by id");
        let outcome = service
            .submit_plan(SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-id-bound-1".into(),
            })
            .await
            .expect("submit");
        match outcome {
            SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
            SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
        }
    }

    #[tokio::test]
    async fn service_rejects_object_graph_mismatch() {
        let service = ExecutorService::new(InMemoryStore::default());
        let normalized = service.normalize(intent()).await.expect("normalize");
        let mut snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        snapshot.normalized_intent_id = "other".into();
        let err = service
            .evaluate_decision(DecisionRequest {
                normalized_intent: normalized,
                snapshot,
            })
            .await
            .expect_err("mismatched snapshot must fail");
        assert!(matches!(err, ServiceError::Conflict(_)));
    }
    #[tokio::test]
    async fn static_runtime_provider_can_reach_ready_plan_but_submit_still_blocks() {
        let service = ExecutorService::with_runtime_provider(
            InMemoryStore::default(),
            StaticRuntimeStateProvider::new(allow_runtime_state()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Allow);
        let plan = service
            .compile_plan_by_id(CompilePlanByIdCommand {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
                decision_id: decision.decision_id.clone(),
                approval: approval(),
            })
            .await
            .expect("plan");
        assert_eq!(plan.status, PlanStatus::Ready);
        let outcome = service
            .submit_plan(SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-ready-still-blocked".into(),
            })
            .await
            .expect("submit");
        match outcome {
            SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
            SubmitOutcome::Replayed(_) => panic!("first submit should not replay"),
        }
    }

    #[tokio::test]
    async fn service_validates_and_persists_sign_only_lifecycle_sequence() {
        let store = InMemoryStore::default();
        let service = ExecutorService::new(store.clone());
        let execution_id = ExecutionId("exec-sign-only-service".into());
        let account_id = AccountId("acct-sign-only-service".into());
        seed_test_plan(&store, &execution_id.0, &account_id.0).await;
        for (event, state, signed_order_ref) in [
            (
                SignOnlyLifecycleEventKind::PrepareReservation,
                SignOnlyLifecycleState::ReservationPrepared,
                None,
            ),
            (
                SignOnlyLifecycleEventKind::RequestSigning,
                SignOnlyLifecycleState::SigningRequested,
                None,
            ),
            (
                SignOnlyLifecycleEventKind::SignedWithoutPost,
                SignOnlyLifecycleState::SignedDryRun,
                Some("sign-only:redacted-ref".to_string()),
            ),
        ] {
            service
                .record_sign_only_lifecycle_event(SignOnlyLifecycleRecord {
                    execution_id: execution_id.clone(),
                    account_id: account_id.clone(),
                    state,
                    event,
                    client_event_id: None,
                    signed_order_ref,
                    no_remote_side_effect: true,
                    event_id: None,
                    created_at: None,
                })
                .await
                .expect("record sign-only lifecycle");
        }
        let records = service
            .list_sign_only_lifecycle_events(SignOnlyLifecycleQuery {
                execution_id: execution_id.0.clone(),
                limit: 100,
                before_event_id: None,
            })
            .await
            .expect("list sign-only lifecycle");
        assert_eq!(records.len(), 3);
        assert_eq!(
            records.last().unwrap().state,
            SignOnlyLifecycleState::SignedDryRun
        );
    }

    #[tokio::test]
    async fn service_records_standard_sign_only_construction_without_raw_payload() {
        let store = InMemoryStore::default();
        let service = ExecutorService::new(store.clone());
        seed_test_plan(&store, "exec-sdk-standard", "acct-sdk-standard").await;

        let receipt = service
            .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
                execution_id: "exec-sdk-standard".into(),
                account_id: "acct-sdk-standard".into(),
                plan_hash: "hash-exec-sdk-standard".into(),
                signed_order_ref: "sign-only:digest-ref".into(),
                no_remote_side_effect: true,
            })
            .await
            .expect("record standard sign-only construction");

        assert!(receipt.no_remote_side_effect);
        assert_eq!(receipt.lifecycle_records.len(), 3);
        assert_eq!(
            receipt.lifecycle_records.last().unwrap().state,
            SignOnlyLifecycleState::SignedDryRun
        );
        assert_eq!(
            receipt
                .lifecycle_records
                .last()
                .unwrap()
                .signed_order_ref
                .as_deref(),
            Some("sign-only:digest-ref")
        );
    }

    #[tokio::test]
    async fn service_rejects_sign_only_sequence_mismatch() {
        let store = InMemoryStore::default();
        let service = ExecutorService::new(store.clone());
        seed_test_plan(&store, "exec-sign-only-bad", "acct-sign-only-bad").await;
        let err = service
            .record_sign_only_lifecycle_event(SignOnlyLifecycleRecord {
                execution_id: ExecutionId("exec-sign-only-bad".into()),
                account_id: AccountId("acct-sign-only-bad".into()),
                state: SignOnlyLifecycleState::SignedDryRun,
                event: SignOnlyLifecycleEventKind::SignedWithoutPost,
                client_event_id: None,
                signed_order_ref: Some("sign-only:redacted-ref".into()),
                no_remote_side_effect: true,
                event_id: None,
                created_at: None,
            })
            .await
            .expect_err("cannot sign without reservation/signing request");
        assert!(matches!(err, ServiceError::Conflict(_)));
    }

    #[tokio::test]
    async fn store_backed_runtime_provider_uses_store_state() {
        let store = InMemoryStore::default();
        let ready_state = allow_runtime_state();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, ready_state);
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            store
                .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                    worker_id: format!("worker-{capability}"),
                    role: "service-test".into(),
                    capability: capability.into(),
                    status: "HEALTHY".into(),
                    last_heartbeat_at: Utc::now(),
                    last_error: None,
                })
                .await
                .expect("record worker heartbeat");
        }
        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        assert_eq!(
            snapshot.runtime_state.geoblock_status,
            GeoblockStatus::Allowed
        );
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Healthy);
        assert_eq!(
            snapshot.runtime_state.required_capabilities,
            vec![
                "heartbeat".to_string(),
                "reconcile".to_string(),
                "resource-refresh".to_string(),
            ]
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Allow);
    }

    #[tokio::test]
    async fn service_records_runtime_worker_signals_for_decision_gate() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            store
                .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                    worker_id: format!("worker-{capability}"),
                    role: "service-test".into(),
                    capability: capability.into(),
                    status: "HEALTHY".into(),
                    last_heartbeat_at: Utc::now(),
                    last_error: None,
                })
                .await
                .expect("record worker heartbeat");
        }
        let recorded = record_runtime_worker_signals(
            &store,
            "acct-1",
            &[RuntimeSignal::HeartbeatLease {
                active: false,
                last_observed_at: Some(Utc::now()),
                last_error: Some("lease expired".into()),
            }],
        )
        .await
        .expect("record runtime worker signal");
        assert_eq!(recorded, 1);

        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"heartbeat-lease".to_string())
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerStale));
    }

    #[tokio::test]
    async fn service_records_runtime_worker_tick_heartbeat_and_observations() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
        let receipt = record_runtime_worker_tick(
            &store,
            "acct-1",
            RuntimeWorkerTick {
                worker_id: "worker-websocket-market".into(),
                role: "WebSocketLiveness".into(),
                capability: "websocket:market".into(),
                status: "HEALTHY".into(),
                last_error: None,
                signals: vec![RuntimeSignal::WebSocket {
                    channel: pmx_runtime::WebSocketChannel::Market,
                    connected: false,
                    stale: true,
                    last_observed_at: Some(Utc::now()),
                    last_error: Some("market websocket disconnected".into()),
                }],
            },
        )
        .await
        .expect("record runtime worker tick");
        assert!(receipt.heartbeat_recorded);
        assert_eq!(receipt.observations_recorded, 1);

        let state = store
            .load_runtime_state(&RuntimeStateQuery {
                account_id: "acct-1".into(),
                condition_id: "cond-1".into(),
                collateral_profile_id: None,
                required_capabilities: vec!["websocket:market".into()],
            })
            .await
            .expect("runtime state");
        assert_eq!(state.worker_status, WorkerStatus::Degraded);

        let normalized = ExecutorService::new(store.clone())
            .normalize(intent())
            .await
            .expect("normalize");
        let decision = evaluate_constraints(
            &normalized,
            &FeasibilitySnapshot {
                snapshot_id: "snapshot-worker-tick".into(),
                snapshot_hash: HashValue("snapshot-hash-worker-tick".into()),
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                runtime_state: state,
                captured_at: Utc::now(),
            },
        );
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
    }

    #[tokio::test]
    async fn service_records_runtime_worker_provider_snapshot_for_decision_gate() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            store
                .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                    worker_id: format!("worker-{capability}"),
                    role: "service-test".into(),
                    capability: capability.into(),
                    status: "HEALTHY".into(),
                    last_heartbeat_at: Utc::now(),
                    last_error: None,
                })
                .await
                .expect("record worker heartbeat");
        }

        let receipt = record_runtime_worker_provider_snapshot(
            &store,
            pmx_runtime::RuntimeWorkerProviderSnapshot {
                account_id: "acct-1".into(),
                lease_owner_id: "worker-runtime-1".into(),
                instance_id: "worker-runtime-2".into(),
                market_websocket_connected: true,
                market_websocket_stale: false,
                user_websocket_connected: true,
                user_websocket_stale: false,
                geoblock_status: GeoblockStatus::Allowed,
                resource_refresh_fresh: true,
                remote_unknown_orders: 0,
                observed_at: Utc::now(),
                provider_name: "real-runtime-provider-test".into(),
                no_trading_side_effect: true,
            },
        )
        .await
        .expect("record provider snapshot");
        assert!(receipt.heartbeat_recorded);
        assert!(!receipt.lease_owner_active);
        assert!(!receipt.submit_allowed_by_runtime);
        assert_eq!(receipt.observations_recorded, 6);

        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"heartbeat-lease".to_string())
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerStale));
    }

    #[tokio::test]
    async fn service_records_heartbeat_lease_election_tick_fail_closed_for_non_owner() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            store
                .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                    worker_id: format!("worker-{capability}"),
                    role: "service-test".into(),
                    capability: capability.into(),
                    status: "HEALTHY".into(),
                    last_heartbeat_at: Utc::now(),
                    last_error: None,
                })
                .await
                .expect("record worker heartbeat");
        }

        let observed_at = Utc::now();
        let receipt = record_heartbeat_lease_election_tick(
            &store,
            HeartbeatLeaseElectionTick {
                account_id: "acct-1".into(),
                provider_name: "heartbeat-lease-election-test".into(),
                instance_id: "worker-b".into(),
                observed_at,
                stale_after_seconds: 30,
                no_trading_side_effect: true,
                candidates: vec![
                    HeartbeatLeaseCandidate {
                        worker_id: "worker-a".into(),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_heartbeat_at: observed_at - chrono::Duration::seconds(1),
                        last_error: None,
                    },
                    HeartbeatLeaseCandidate {
                        worker_id: "worker-b".into(),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_heartbeat_at: observed_at - chrono::Duration::seconds(2),
                        last_error: None,
                    },
                ],
            },
        )
        .await
        .expect("record heartbeat lease election tick");
        assert_eq!(receipt.election.lease_owner_id, "worker-a");
        assert!(receipt.election.fail_closed);
        assert!(!receipt.provider_tick.lease_owner_active);

        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
    }

    #[tokio::test]
    async fn service_records_resource_refresh_worker_tick_for_decision_gate() {
        let store = InMemoryStore::default();
        store.set_runtime_state_for_test("acct-1", "cond-1", None, allow_runtime_state());
        for capability in ["heartbeat", "reconcile", "resource-refresh"] {
            store
                .record_worker_heartbeat(&RuntimeWorkerHeartbeat {
                    worker_id: format!("worker-{capability}"),
                    role: "service-test".into(),
                    capability: capability.into(),
                    status: "HEALTHY".into(),
                    last_heartbeat_at: Utc::now(),
                    last_error: None,
                })
                .await
                .expect("record worker heartbeat");
        }

        let observed_at = Utc::now();
        let receipt = record_resource_refresh_worker_tick(
            &store,
            ResourceRefreshWorkerTick {
                account_id: "acct-1".into(),
                provider_name: "resource-refresh-worker-test".into(),
                instance_id: "worker-resource-refresh".into(),
                lease_owner_id: "worker-resource-refresh".into(),
                market_websocket_connected: true,
                market_websocket_stale: false,
                user_websocket_connected: true,
                user_websocket_stale: false,
                geoblock_status: GeoblockStatus::Allowed,
                remote_unknown_orders: 0,
                observed_at,
                stale_after_seconds: 30,
                no_trading_side_effect: true,
                observations: vec![
                    pmx_runtime::ResourceRefreshObservation {
                        component: pmx_runtime::ResourceRefreshComponent::Account,
                        resource_id: "acct-1".into(),
                        refreshed_at: observed_at - chrono::Duration::seconds(60),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_error: None,
                    },
                    pmx_runtime::ResourceRefreshObservation {
                        component: pmx_runtime::ResourceRefreshComponent::Market,
                        resource_id: "cond-1".into(),
                        refreshed_at: observed_at - chrono::Duration::seconds(5),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_error: None,
                    },
                ],
            },
        )
        .await
        .expect("record resource refresh worker tick");
        assert!(!receipt.evaluation.fresh);
        assert_eq!(receipt.evaluation.stale_components, vec!["account:acct-1"]);
        assert!(receipt.provider_tick.lease_owner_active);
        assert!(!receipt.provider_tick.submit_allowed_by_runtime);

        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized.clone())
            .await
            .expect("snapshot");
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Stale);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"resource-refresh".to_string())
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerStale));
    }

    #[tokio::test]
    async fn service_records_non_live_cancel_and_reconcile_order_lifecycle() {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&order("order-non-live-cancel", OrderLifecycleState::Posted))
            .await
            .expect("upsert order");
        let service = ExecutorService::new(store.clone());

        let canceled = service
            .record_non_live_cancel_request(
                "order-non-live-cancel",
                "operator requested cancel",
                Some("corr-cancel".into()),
            )
            .await
            .expect("record cancel")
            .expect("existing order");
        assert_eq!(
            canceled.lifecycle_state,
            OrderLifecycleState::CancelRequested
        );

        store
            .upsert_order_lifecycle(&order(
                "order-non-live-reconcile",
                OrderLifecycleState::RemoteUnknown,
            ))
            .await
            .expect("upsert remote unknown order");
        let reconciled = service
            .record_non_live_reconcile_observation(
                "order-non-live-reconcile",
                OrderEventKind::ReconcileMissing,
                "remote missing in drill",
                Some("corr-reconcile".into()),
            )
            .await
            .expect("record reconcile")
            .expect("existing order");
        assert_eq!(
            reconciled.lifecycle_state,
            OrderLifecycleState::PartialRemoteUnknown
        );

        let missing = service
            .record_non_live_cancel_request("missing-order", "operator requested cancel", None)
            .await
            .expect("missing order is non-fatal");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn service_classifies_and_records_order_lifecycle_divergence_without_remote_side_effect()
    {
        let store = InMemoryStore::default();
        store
            .upsert_order_lifecycle(&order(
                "order-divergence",
                OrderLifecycleState::RemoteUnknown,
            ))
            .await
            .expect("upsert order");
        let service = ExecutorService::new(store.clone());

        let (first_divergence, first_update) = service
            .reconcile_order_lifecycle_divergence(
                "order-divergence",
                Some("acct-1"),
                RemoteOrderObservation::Missing,
                "remote read observed missing",
                Some("corr-divergence-1".into()),
            )
            .await
            .expect("first divergence")
            .expect("order exists");
        assert_eq!(
            first_divergence.kind,
            OrderLifecycleDivergenceKind::LocalRemoteUnknownRemoteMissing
        );
        assert!(!first_divergence.operator_required);
        assert!(first_divergence.no_remote_side_effect);
        assert_eq!(
            first_update.expect("first update").lifecycle_state,
            OrderLifecycleState::PartialRemoteUnknown
        );

        let (second_divergence, second_update) = service
            .reconcile_order_lifecycle_divergence(
                "order-divergence",
                Some("acct-1"),
                RemoteOrderObservation::Missing,
                "remote read still missing",
                Some("corr-divergence-2".into()),
            )
            .await
            .expect("second divergence")
            .expect("order exists");
        assert!(second_divergence.operator_required);
        assert_eq!(
            second_update.expect("second update").lifecycle_state,
            OrderLifecycleState::Failed
        );

        let events = store
            .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
                order_id: "order-divergence".into(),
                limit: 10,
                before_event_id: None,
            })
            .await
            .expect("order lifecycle events");
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.payload["no_remote_side_effect"] == true)
        );
    }
}
