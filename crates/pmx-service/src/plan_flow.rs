use chrono::Utc;
use pmx_core::{
    ApprovalReceipt, ConstraintDecision, DecimalString, DecisionStatus, ExecutionPlanSummary,
    FeasibilitySnapshot, HashValue, NormalizedIntent, PlanStatus, TradeIntent,
    canonical_json_sha256, normalize_intent,
};
use pmx_policy::evaluate_constraints;
use pmx_store::ExecutionStore;
use uuid::Uuid;

use crate::binding::{
    PlanHashInput, SnapshotHashInput, verify_decision_binding, verify_snapshot_binding,
};
use crate::{
    CompilePlanByIdCommand, CompilePlanCommand, DecisionByIdRequest, DecisionRequest,
    RuntimeStateProvider, ServiceError,
};

pub async fn normalize<S>(store: &S, intent: TradeIntent) -> Result<NormalizedIntent, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized =
        normalize_intent(intent).map_err(|err| ServiceError::BadRequest(err.to_string()))?;
    store.save_normalized_intent(&normalized).await?;
    Ok(normalized)
}

pub async fn capture_snapshot<S, R>(
    store: &S,
    runtime_state_provider: &R,
    normalized: NormalizedIntent,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    S: ExecutionStore + Send + Sync,
    R: RuntimeStateProvider,
{
    store.save_normalized_intent(&normalized).await?;
    let snapshot = build_snapshot(runtime_state_provider, &normalized).await?;
    store.save_snapshot(&snapshot).await?;
    Ok(snapshot)
}

pub async fn evaluate_decision<S>(
    store: &S,
    req: DecisionRequest,
) -> Result<ConstraintDecision, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
    store.save_normalized_intent(&req.normalized_intent).await?;
    store.save_snapshot(&req.snapshot).await?;
    let decision = evaluate_constraints(&req.normalized_intent, &req.snapshot);
    store.save_decision(&decision).await?;
    Ok(decision)
}

pub async fn evaluate_decision_by_id<S>(
    store: &S,
    req: DecisionByIdRequest,
) -> Result<ConstraintDecision, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized = store
        .load_normalized_intent(&req.normalized_intent_id)
        .await?;
    let snapshot = store.load_snapshot(&req.snapshot_id).await?;
    evaluate_decision(
        store,
        DecisionRequest {
            normalized_intent: normalized,
            snapshot,
        },
    )
    .await
}

pub async fn compile_plan<S>(
    store: &S,
    req: CompilePlanCommand,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    verify_snapshot_binding(&req.normalized_intent, &req.snapshot)?;
    verify_decision_binding(&req.normalized_intent, &req.snapshot, &req.decision)?;
    store.save_normalized_intent(&req.normalized_intent).await?;
    store.save_snapshot(&req.snapshot).await?;
    store.save_decision(&req.decision).await?;
    build_and_save_plan(
        store,
        &req.normalized_intent,
        &req.snapshot,
        &req.decision,
        &req.approval,
    )
    .await
}

pub async fn compile_plan_by_id<S>(
    store: &S,
    req: CompilePlanByIdCommand,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
    let normalized = store
        .load_normalized_intent(&req.normalized_intent_id)
        .await?;
    let snapshot = store.load_snapshot(&req.snapshot_id).await?;
    let decision = store.load_decision(&req.decision_id).await?;
    verify_snapshot_binding(&normalized, &snapshot)?;
    verify_decision_binding(&normalized, &snapshot, &decision)?;
    build_and_save_plan(store, &normalized, &snapshot, &decision, &req.approval).await
}

async fn build_and_save_plan<S>(
    store: &S,
    normalized: &NormalizedIntent,
    snapshot: &FeasibilitySnapshot,
    decision: &ConstraintDecision,
    approval: &ApprovalReceipt,
) -> Result<ExecutionPlanSummary, ServiceError>
where
    S: ExecutionStore + Send + Sync,
{
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
    store.save_plan_summary(&plan).await?;
    Ok(plan)
}

async fn build_snapshot<R>(
    runtime_state_provider: &R,
    normalized: &NormalizedIntent,
) -> Result<FeasibilitySnapshot, ServiceError>
where
    R: RuntimeStateProvider,
{
    let snapshot_id = Uuid::new_v4().to_string();
    let runtime_state = runtime_state_provider
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
