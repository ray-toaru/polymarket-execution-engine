use async_trait::async_trait;
use chrono::Utc;
use pmx_core::*;
use pmx_policy::evaluate_constraints;
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyAction,
    IdempotencyStore, RuntimeStateQuery, RuntimeStateStore, SignOnlyLifecycleQuery,
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

#[async_trait]
pub trait RuntimeStateProvider: Clone + Send + Sync + 'static {
    async fn capture_runtime_state(
        &self,
        normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary;
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
    use pmx_store::{InMemoryStore, RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat};

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
}
