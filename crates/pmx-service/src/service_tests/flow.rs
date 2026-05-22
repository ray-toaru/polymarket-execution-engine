use super::*;

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
    let events = service
        .list_execution_lifecycle_events(pmx_store::ExecutionLifecycleQuery {
            execution_id: plan.execution_id.clone(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("lifecycle events");
    let blocked = events
        .iter()
        .find(|event| event.event_type == "SUBMIT_BLOCKED_BEFORE_REMOTE")
        .expect("blocked event");
    assert_eq!(blocked.payload["reservation_written"], false);
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
    assert_eq!(plan.max_exposure, DecimalString("1".into()));
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
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    }
}
