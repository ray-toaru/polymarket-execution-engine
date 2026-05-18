use chrono::Utc;
use pmx_core::*;
use pmx_policy::evaluate_constraints;
use pmx_store::{
    AdminAuditEvent, AdminAuditQuery, AdminAuditStore, ExecutionLifecycleEvent,
    ExecutionLifecycleQuery, ExecutionLifecycleStore, ExecutionStore, IdempotencyStore,
    OrderLifecycleEventRecord, OrderLifecycleRecord, OrderLifecycleStore, RuntimeWorkerStatusQuery,
    RuntimeWorkerStatusReport, RuntimeWorkerStatusStore, SignOnlyLifecycleQuery,
    SignOnlyLifecycleStore,
};
use uuid::Uuid;

use crate::binding::{
    PlanHashInput, SnapshotHashInput, verify_decision_binding, verify_snapshot_binding,
};
use crate::model::*;
use crate::runtime_state::{FailClosedRuntimeStateProvider, RuntimeStateProvider};

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
        + RuntimeWorkerStatusStore
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
        + RuntimeWorkerStatusStore
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

    pub async fn list_order_lifecycle_events(
        &self,
        query: pmx_store::OrderLifecycleEventQuery,
    ) -> Result<Vec<OrderLifecycleEventRecord>, ServiceError> {
        Ok(self.store.list_order_lifecycle_events(&query).await?)
    }

    pub async fn list_runtime_worker_status(
        &self,
        query: RuntimeWorkerStatusQuery,
    ) -> Result<RuntimeWorkerStatusReport, ServiceError> {
        if query.account_id.trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "account_id must be non-empty".into(),
            ));
        }
        Ok(self.store.list_runtime_worker_status(&query).await?)
    }

    pub async fn record_non_live_cancel_request(
        &self,
        order_id: &str,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        crate::order_lifecycle::record_non_live_cancel_request(
            &self.store,
            order_id,
            reason,
            correlation_id,
        )
        .await
    }

    pub async fn record_non_live_reconcile_observation(
        &self,
        order_id: &str,
        event: OrderEventKind,
        reason: &str,
        correlation_id: Option<String>,
    ) -> Result<Option<OrderLifecycleRecord>, ServiceError> {
        crate::order_lifecycle::record_non_live_reconcile_observation(
            &self.store,
            order_id,
            event,
            reason,
            correlation_id,
        )
        .await
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
        crate::order_lifecycle::reconcile_order_lifecycle_divergence(
            &self.store,
            order_id,
            account_id,
            remote_observation,
            reason,
            correlation_id,
        )
        .await
    }

    pub async fn record_sign_only_lifecycle_event(
        &self,
        record: SignOnlyLifecycleRecord,
    ) -> Result<SignOnlyLifecycleRecord, ServiceError> {
        crate::sign_only::record_sign_only_lifecycle_event(&self.store, record).await
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
        crate::sign_only::record_standard_sign_only_construction(&self.store, req).await
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
        crate::submit::submit_plan(
            &self.store,
            req,
            &self.executor_version,
            &self.contract_version,
        )
        .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use crate::{StaticRuntimeStateProvider, StoreBackedRuntimeStateProvider};
    use pmx_runtime::{HeartbeatLeaseCandidate, RuntimeSignal};
    use pmx_store::{
        InMemoryStore, OrderLifecycleStore, RuntimeStateQuery, RuntimeStateStore,
        RuntimeWorkerHealthStore, RuntimeWorkerHeartbeat,
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
                signed_order_digest: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                ),
                no_remote_side_effect: true,
            })
            .await
            .expect("record standard sign-only construction");

        assert!(receipt.no_remote_side_effect);
        assert_eq!(
            receipt.signed_order_digest.as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
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
    async fn service_rejects_malformed_standard_sign_only_digest() {
        let store = InMemoryStore::default();
        let service = ExecutorService::new(store.clone());
        seed_test_plan(
            &store,
            "exec-sdk-standard-bad-digest",
            "acct-sdk-standard-bad-digest",
        )
        .await;

        let err = service
            .record_standard_sign_only_construction(StandardSignOnlyConstructionRequest {
                execution_id: "exec-sdk-standard-bad-digest".into(),
                account_id: "acct-sdk-standard-bad-digest".into(),
                plan_hash: "hash-exec-sdk-standard-bad-digest".into(),
                signed_order_ref: "sign-only:digest-ref".into(),
                signed_order_digest: Some("not-a-sha256".into()),
                no_remote_side_effect: true,
            })
            .await
            .expect_err("malformed digest must be rejected");

        assert!(matches!(err, ServiceError::BadRequest(_)));
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
    async fn service_lists_runtime_worker_status() {
        let store = InMemoryStore::default();
        let observed_at = Utc::now();
        record_runtime_worker_tick(
            &store,
            "acct-1",
            RuntimeWorkerTick {
                worker_id: "worker-status-query".into(),
                role: "HeartbeatLease".into(),
                capability: "heartbeat".into(),
                status: "HEALTHY".into(),
                last_error: None,
                signals: vec![RuntimeSignal::HeartbeatLease {
                    active: false,
                    last_observed_at: Some(observed_at),
                    last_error: Some("lease expired".into()),
                }],
            },
        )
        .await
        .expect("record worker tick");
        let service = ExecutorService::new(store);
        let report = service
            .list_runtime_worker_status(RuntimeWorkerStatusQuery {
                account_id: "acct-1".into(),
                limit: 100,
                before_observed_at: None,
            })
            .await
            .expect("list runtime worker status");
        assert_eq!(report.heartbeats.len(), 1);
        assert_eq!(report.heartbeats[0].worker_id, "worker-status-query");
        assert_eq!(report.observations.len(), 1);
        assert_eq!(report.observations[0].capability, "heartbeat-lease");
        assert!(report.observations[0].should_fail_closed);
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
    async fn service_records_continuous_runtime_worker_ticks_fail_closed_on_any_bad_snapshot() {
        let store = InMemoryStore::default();
        let observed_at = Utc::now();
        let healthy_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
            account_id: "acct-1".into(),
            lease_owner_id: "worker-runtime-1".into(),
            instance_id: "worker-runtime-1".into(),
            market_websocket_connected: true,
            market_websocket_stale: false,
            user_websocket_connected: true,
            user_websocket_stale: false,
            geoblock_status: GeoblockStatus::Allowed,
            resource_refresh_fresh: true,
            remote_unknown_orders: 0,
            observed_at,
            provider_name: "runtime-provider-test".into(),
            no_trading_side_effect: true,
        };
        let stale_snapshot = pmx_runtime::RuntimeWorkerProviderSnapshot {
            instance_id: "worker-runtime-2".into(),
            market_websocket_stale: true,
            observed_at: observed_at + chrono::Duration::seconds(1),
            ..healthy_snapshot.clone()
        };

        let receipt = record_runtime_worker_continuous_tick(
            &store,
            RuntimeWorkerContinuousTick {
                snapshots: vec![healthy_snapshot, stale_snapshot],
                no_trading_side_effect: true,
            },
        )
        .await
        .expect("record continuous runtime ticks");

        assert_eq!(receipt.ticks_recorded.len(), 2);
        assert!(receipt.ticks_recorded[0].submit_allowed_by_runtime);
        assert!(!receipt.ticks_recorded[1].submit_allowed_by_runtime);
        assert!(!receipt.all_submit_allowed_by_runtime);
        assert!(receipt.no_trading_side_effect);

        let report = store
            .list_runtime_worker_status(&RuntimeWorkerStatusQuery {
                account_id: "acct-1".into(),
                limit: 100,
                before_observed_at: None,
            })
            .await
            .expect("list runtime worker status");
        assert_eq!(report.heartbeats.len(), 2);
        assert!(
            report
                .observations
                .iter()
                .any(|observation| observation.capability == "websocket:market"
                    && observation.should_fail_closed)
        );
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
    async fn service_records_reconcile_backlog_worker_tick_for_decision_gate() {
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
        let receipt = record_reconcile_backlog_worker_tick(
            &store,
            ReconcileBacklogWorkerTick {
                account_id: "acct-1".into(),
                provider_name: "reconcile-backlog-worker-test".into(),
                instance_id: "worker-reconcile-backlog".into(),
                lease_owner_id: "worker-reconcile-backlog".into(),
                market_websocket_connected: true,
                market_websocket_stale: false,
                user_websocket_connected: true,
                user_websocket_stale: false,
                geoblock_status: GeoblockStatus::Allowed,
                resource_refresh_fresh: true,
                remote_unknown_order_ids: vec!["order-remote-unknown".into()],
                observed_at,
                no_trading_side_effect: true,
            },
        )
        .await
        .expect("record reconcile backlog worker tick");
        assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
        assert!(receipt.evaluation.submit_blocked);
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
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"reconcile-backlog".to_string())
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
    }

    #[tokio::test]
    async fn service_records_reconcile_backlog_from_order_lifecycle() {
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
        store
            .upsert_order_lifecycle(&order(
                "order-lifecycle-backlog",
                OrderLifecycleState::RemoteUnknown,
            ))
            .await
            .expect("upsert remote unknown order");
        store
            .upsert_order_lifecycle(&order(
                "order-lifecycle-posted",
                OrderLifecycleState::Posted,
            ))
            .await
            .expect("upsert posted order");

        let observed_at = Utc::now();
        let receipt = record_reconcile_backlog_from_order_lifecycle(
            &store,
            ReconcileBacklogWorkerTick {
                account_id: "acct-1".into(),
                provider_name: "reconcile-lifecycle-reader-test".into(),
                instance_id: "worker-reconcile-lifecycle-reader".into(),
                lease_owner_id: "worker-reconcile-lifecycle-reader".into(),
                market_websocket_connected: true,
                market_websocket_stale: false,
                user_websocket_connected: true,
                user_websocket_stale: false,
                geoblock_status: GeoblockStatus::Allowed,
                resource_refresh_fresh: true,
                remote_unknown_order_ids: vec![],
                observed_at,
                no_trading_side_effect: true,
            },
        )
        .await
        .expect("record reconcile backlog from order lifecycle");
        assert_eq!(receipt.evaluation.remote_unknown_orders, 1);
        assert!(receipt.evaluation.submit_blocked);
        assert!(!receipt.provider_tick.submit_allowed_by_runtime);

        let service = ExecutorService::with_runtime_provider(
            store.clone(),
            StoreBackedRuntimeStateProvider::new(store.clone()),
            "test-executor".into(),
            DEFAULT_CONTRACT_VERSION.into(),
        );
        let normalized = service.normalize(intent()).await.expect("normalize");
        let snapshot = service
            .capture_snapshot(normalized)
            .await
            .expect("snapshot");
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"reconcile-backlog".to_string())
        );
    }

    #[tokio::test]
    async fn service_records_websocket_liveness_worker_tick_for_decision_gate() {
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
        let receipt = record_websocket_liveness_worker_tick(
            &store,
            WebSocketLivenessWorkerTick {
                account_id: "acct-1".into(),
                provider_name: "websocket-liveness-worker-test".into(),
                instance_id: "worker-websocket-liveness".into(),
                lease_owner_id: "worker-websocket-liveness".into(),
                geoblock_status: GeoblockStatus::Allowed,
                resource_refresh_fresh: true,
                remote_unknown_orders: 0,
                observed_at,
                stale_after_seconds: 30,
                no_trading_side_effect: true,
                observations: vec![
                    pmx_runtime::WebSocketLivenessObservation {
                        channel: pmx_runtime::WebSocketChannel::Market,
                        connected: true,
                        last_message_at: Some(observed_at - chrono::Duration::seconds(5)),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_error: None,
                    },
                    pmx_runtime::WebSocketLivenessObservation {
                        channel: pmx_runtime::WebSocketChannel::User,
                        connected: false,
                        last_message_at: None,
                        status: pmx_runtime::HealthLevel::Degraded,
                        last_error: Some("user websocket disconnected".into()),
                    },
                ],
            },
        )
        .await
        .expect("record websocket liveness worker tick");
        assert!(receipt.evaluation.market_connected);
        assert!(!receipt.evaluation.user_connected);
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
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Degraded);
        assert!(
            snapshot
                .runtime_state
                .required_capabilities
                .contains(&"websocket:user".to_string())
        );
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerDegraded));
    }

    #[tokio::test]
    async fn service_records_geoblock_worker_tick_for_decision_gate() {
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

        let receipt = record_geoblock_worker_tick(
            &store,
            GeoblockWorkerTick {
                account_id: "acct-1".into(),
                provider_name: "geoblock-worker-test".into(),
                instance_id: "worker-geoblock".into(),
                lease_owner_id: "worker-geoblock".into(),
                market_websocket_connected: true,
                market_websocket_stale: false,
                user_websocket_connected: true,
                user_websocket_stale: false,
                status: GeoblockStatus::Unknown,
                resource_refresh_fresh: true,
                remote_unknown_orders: 0,
                observed_at: Utc::now(),
                last_error: Some("geoblock provider timeout".into()),
                no_trading_side_effect: true,
            },
        )
        .await
        .expect("record geoblock worker tick");
        assert!(!receipt.evaluation.submit_allowed);
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
        assert_eq!(
            snapshot.runtime_state.geoblock_status,
            GeoblockStatus::Allowed
        );
        assert_eq!(snapshot.runtime_state.worker_status, WorkerStatus::Unknown);
        let decision = service
            .evaluate_decision_by_id(DecisionByIdRequest {
                normalized_intent_id: normalized.normalized_intent_id.clone(),
                snapshot_id: snapshot.snapshot_id.clone(),
            })
            .await
            .expect("decision");
        assert_eq!(decision.status, DecisionStatus::Block);
        assert!(decision.reasons.contains(&BlockReason::WorkerUnknown));
    }

    #[tokio::test]
    async fn service_records_worker_crash_recovery_tick_for_decision_gate() {
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
        let receipt = record_worker_crash_recovery_tick(
            &store,
            WorkerCrashRecoveryTick {
                account_id: "acct-1".into(),
                worker_id: "worker-crash-recovery".into(),
                required_capabilities: vec![
                    "heartbeat".into(),
                    "reconcile".into(),
                    "resource-refresh".into(),
                ],
                observed_at,
                stale_after_seconds: 30,
                no_trading_side_effect: true,
                observations: vec![
                    pmx_runtime::WorkerCrashRecoveryObservation {
                        worker_id: "worker-heartbeat".into(),
                        capability: "heartbeat".into(),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(5)),
                        last_error: None,
                    },
                    pmx_runtime::WorkerCrashRecoveryObservation {
                        worker_id: "worker-reconcile".into(),
                        capability: "reconcile".into(),
                        status: pmx_runtime::HealthLevel::Healthy,
                        last_heartbeat_at: Some(observed_at - chrono::Duration::seconds(60)),
                        last_error: Some("stale after crash".into()),
                    },
                ],
            },
        )
        .await
        .expect("record worker crash recovery tick");
        assert!(receipt.heartbeat_recorded);
        assert!(receipt.observation_recorded);
        assert!(!receipt.evaluation.recovered);
        assert_eq!(receipt.evaluation.stale_workers, vec!["worker-reconcile"]);
        assert_eq!(
            receipt.evaluation.missing_capabilities,
            vec!["resource-refresh"]
        );

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
                .contains(&"worker-crash-recovery".to_string())
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
        assert!(
            events
                .iter()
                .all(|event| event.correlation_id.as_deref().is_some())
        );

        let queried = service
            .list_order_lifecycle_events(pmx_store::OrderLifecycleEventQuery {
                order_id: "order-divergence".into(),
                limit: 10,
                before_event_id: None,
            })
            .await
            .expect("query order lifecycle events");
        assert_eq!(queried, events);
    }
}
